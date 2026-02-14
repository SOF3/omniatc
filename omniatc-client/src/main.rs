fn main() {
    let options = <omniatc_client::Options as clap::Parser>::parse();
    omniatc_client::main_app(options).run();
}
