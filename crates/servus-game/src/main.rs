use std::error::Error;

use servus_game::run_demo;

fn main() -> Result<(), Box<dyn Error>> {
    let result = run_demo()?;

    println!(
        "Built {} | tick={} received={} served={} dropped={} credits={}",
        result.service_name,
        result.report.tick.number(),
        result.report.received,
        result.report.served,
        result.report.dropped,
        result.remaining_credits
    );

    Ok(())
}
