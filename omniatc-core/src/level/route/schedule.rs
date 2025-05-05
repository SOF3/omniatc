use std::collections::VecDeque;
use std::marker::PhantomData;
use std::mem;
use std::ops::DerefMut;

use bevy::ecs::system::EntityCommand;
use bevy::ecs::world::{EntityWorldMut, Mut};

use super::{altitude, heading, Node, NodeResyncResult, Route};

#[derive(Clone, Default)]
pub struct Schedule {
    pub channels:        ChannelsOwned,
    next_condition_id:   usize,
    notified_conditions: Vec<bool>,
}

impl Schedule {
    fn notify_condition(&mut self, condition: ConditionId) {
        self.notified_conditions.resize(condition.0 + 1, false);
        self.notified_conditions[condition.0] = true;
    }

    #[must_use]
    pub fn is_condition_notified(&self, condition: ConditionId) -> bool {
        self.notified_conditions.get(condition.0) == Some(&true)
    }

    pub fn alloc_condition(&mut self) -> ConditionId {
        let out = ConditionId(self.next_condition_id);
        self.next_condition_id += 1;
        out
    }
}

#[derive(Clone, Default)]
pub struct Channels<C: Chan> {
    pub altitude: C::Chan<altitude::Node>,
    pub heading:  C::Chan<heading::Node>,
}

impl<C: Chan> Channels<C> {
    pub fn slice_until_condition<'a>(&'a self, condition: ConditionId) -> Channels<impl Chan + 'a> {
        fn not_cond<T>(condition: ConditionId) -> impl Fn(&Entry<T>) -> bool {
            move |entry| !matches!(*entry, Entry::Wait { until: c } | Entry::Notify { which: c } if c == condition )
        }
        Channels::<SlicedChan<'a>> {
            altitude: self.altitude.slice_until_false(not_cond(condition)),
            heading:  self.heading.slice_until_false(not_cond(condition)),
        }
    }
}

fn channel_vtables() -> [ChannelVtable; 2] {
    [
        ChannelVtable {
            resync_channel_once:   |entity, only_if_waiting| {
                resync_once(entity, only_if_waiting, |channels| &mut channels.altitude)
            },
            get_trigger_condition: |channels| channels.altitude.trigger_condition,
        },
        ChannelVtable {
            resync_channel_once:   |entity, only_if_waiting| {
                resync_once(entity, only_if_waiting, |channels| &mut channels.heading)
            },
            get_trigger_condition: |channels| channels.heading.trigger_condition,
        },
    ]
}

type ChannelsOwned = Channels<OwnedChan>;

pub trait Chan {
    type Chan<T: 'static>: Default + ChannelRef<T>;
}

#[derive(Clone, Default)]
pub struct OwnedChan;

impl Chan for OwnedChan {
    type Chan<T: 'static> = Channel<T>;
}

#[derive(Clone, Default)]
pub struct SlicedChan<'a>(PhantomData<&'a ()>);

impl<'a> Chan for SlicedChan<'a> {
    type Chan<T: 'static> = ChannelSlice<'a, T>;
}

struct ChannelVtable {
    resync_channel_once:   fn(&mut EntityWorldMut, bool) -> ResyncChannelResult,
    get_trigger_condition: fn(&ChannelsOwned) -> Option<ConditionId>,
}

enum ResyncChannelResult {
    NextChannel,
    RetryChannel,
    RetryAll(RetryAllChannelsError),
}

pub trait ChannelRef<T> {
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Entry<T>> + 'a
    where
        T: 'a;

    fn slice_until_false(&self, keep_predicate: impl Fn(&Entry<T>) -> bool) -> ChannelSlice<T>;
}

#[derive(Clone)]
pub struct Channel<T> {
    pub queue:         VecDeque<Entry<T>>,
    trigger_condition: Option<ConditionId>,
}

impl<T> Default for Channel<T> {
    fn default() -> Self { Self { queue: VecDeque::new(), trigger_condition: None } }
}

impl<T> Channel<T> {
    pub fn push_custom(&mut self, entry: impl Into<T>) {
        self.queue.push_back(Entry::Custom(entry.into()));
    }
    pub fn push_wait(&mut self, until: ConditionId) { self.queue.push_back(Entry::Wait { until }); }
    pub fn push_notify(&mut self, which: ConditionId) {
        self.queue.push_back(Entry::Notify { which });
    }
}

fn slice_slices_until_false<'a, T>(
    front: &'a [T],
    back: &'a [T],
    keep_predicate: impl Fn(&T) -> bool,
) -> [&'a [T]; 2] {
    if let Some(pos) = front.iter().chain(back).position(|t| !keep_predicate(t)) {
        if let Some(back_len) = pos.checked_sub(front.len()) {
            [front, &back[..back_len]]
        } else {
            [&front[..pos], &[]]
        }
    } else {
        [front, back]
    }
}

impl<T> ChannelRef<T> for Channel<T> {
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Entry<T>> + 'a
    where
        T: 'a,
    {
        self.queue.iter()
    }

    fn slice_until_false(&self, keep_predicate: impl Fn(&Entry<T>) -> bool) -> ChannelSlice<T> {
        let (front, back) = self.queue.as_slices();
        ChannelSlice(slice_slices_until_false(front, back, keep_predicate))
    }
}

pub struct ChannelSlice<'a, T>([&'a [Entry<T>]; 2]);

impl<T> Default for ChannelSlice<'_, T> {
    fn default() -> Self { Self([&[], &[]]) }
}

impl<T> ChannelRef<T> for ChannelSlice<'_, T> {
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Entry<T>> + 'a
    where
        T: 'a,
    {
        self.0.iter().flat_map(|&slice| slice)
    }

    fn slice_until_false(&self, keep_predicate: impl Fn(&Entry<T>) -> bool) -> ChannelSlice<T> {
        ChannelSlice(slice_slices_until_false(self.0[0], self.0[1], keep_predicate))
    }
}

fn schedule_from_entity<'a>(
    entity: &'a mut EntityWorldMut,
) -> impl DerefMut<Target = Schedule> + 'a {
    Mut::map_unchanged(
        entity.get_mut::<Route>().expect("cannot resync entity without schedule"),
        |r| &mut r.schedule,
    )
}

fn resync_once<T: Node>(
    entity: &mut EntityWorldMut,
    only_if_waiting: bool,
    field_mut: impl Fn(&mut ChannelsOwned) -> &mut Channel<T>,
) -> ResyncChannelResult {
    let mut channel = {
        let mut schedule = schedule_from_entity(entity);
        if only_if_waiting {
            let trigger_condition = field_mut(&mut schedule.channels).trigger_condition;
            match trigger_condition {
                None => return ResyncChannelResult::NextChannel,
                Some(waiting) if !schedule.is_condition_notified(waiting) => {
                    return ResyncChannelResult::NextChannel
                }
                _ => {}
            }
        }
        mem::take(field_mut(&mut schedule.channels))
    };
    let result = resync_once_with(entity, &mut channel);
    *field_mut(&mut schedule_from_entity(entity).channels) = channel;
    result
}

fn resync_once_with<T: Node>(
    entity: &mut EntityWorldMut,
    channel: &mut Channel<T>,
) -> ResyncChannelResult {
    channel.trigger_condition = None;

    match channel.queue.front_mut() {
        None => ResyncChannelResult::NextChannel,
        Some(Entry::Custom(node)) => match node.resync(entity) {
            NodeResyncResult::Pending => ResyncChannelResult::NextChannel,
            NodeResyncResult::Completed => {
                node.teardown(entity);
                channel.queue.pop_front().expect("front_mut is some");
                ResyncChannelResult::RetryChannel
            }
            NodeResyncResult::Interrupt => {
                node.teardown(entity);
                ResyncChannelResult::RetryAll(RetryAllChannelsError::Interrupt)
            }
        },
        Some(&mut Entry::Wait { until })
            if schedule_from_entity(entity).is_condition_notified(until) =>
        {
            channel.queue.pop_front().expect("front_mut is some");
            ResyncChannelResult::RetryChannel
        }
        Some(&mut Entry::Wait { until }) => {
            channel.trigger_condition = Some(until);
            ResyncChannelResult::NextChannel
        }
        Some(&mut Entry::Notify { which }) => {
            schedule_from_entity(entity).notify_condition(which);
            ResyncChannelResult::RetryAll(RetryAllChannelsError::NewNotify)
        }
    }
}

#[derive(Clone)]
pub enum Entry<T> {
    /// Notify completion of `NotifyId` upon execution.
    /// Completes immediately.
    Notify {
        which: ConditionId,
    },
    /// Completes when `until` is notified.
    Wait {
        until: ConditionId,
    },
    Custom(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionId(pub usize);

pub struct DoResync;

impl EntityCommand for DoResync {
    fn apply(self, mut entity: EntityWorldMut) {
        let mut only_if_waiting = false;
        loop {
            match resync_all_channels(&mut entity, only_if_waiting) {
                Ok(()) => break,
                Err(RetryAllChannelsError::NewNotify) => {
                    only_if_waiting = true;
                }
                Err(RetryAllChannelsError::Interrupt) => {
                    only_if_waiting = false;
                }
            }
        }
    }
}

enum RetryAllChannelsError {
    /// Resync all channels due ot trigger change.
    NewNotify,
    /// Resync all channels due to a [node interrupt](super::NodeResyncResult::Interrupt).
    Interrupt,
}

fn resync_all_channels(
    entity: &mut EntityWorldMut,
    only_if_waiting: bool,
) -> Result<(), RetryAllChannelsError> {
    for channel in channel_vtables() {
        resync_channel(entity, channel, only_if_waiting)?;
    }

    Ok(())
}

fn resync_channel(
    entity: &mut EntityWorldMut,
    channel: ChannelVtable,
    only_if_waiting: bool,
) -> Result<(), RetryAllChannelsError> {
    loop {
        match (channel.resync_channel_once)(entity, only_if_waiting) {
            ResyncChannelResult::NextChannel => break Ok(()),
            ResyncChannelResult::RetryChannel => {}
            ResyncChannelResult::RetryAll(err) => break Err(err),
        }
    }
}
