use anyhow::Result;
use programmable_parameter_demo::demo::run_demo;

fn main() -> Result<()> {
    let report = run_demo()?;
    report.print();
    Ok(())
}
