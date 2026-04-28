use achitek_ls::{Server, arguments};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn init_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false),
        )
        .init();
}

fn main() -> anyhow::Result<()> {
    let args = arguments::parse()?;

    init_logging();

    let server = Server::new(args.channel);

    server.run()?;

    Ok(())
}
