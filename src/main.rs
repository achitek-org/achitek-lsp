use achitek_ls::arguments;

fn main() -> Result<(), lexopt::Error> {
    let _args = arguments::parse()?;

    Ok(())
}
