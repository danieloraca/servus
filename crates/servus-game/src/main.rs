use std::error::Error;

use servus_game::run_demo;

fn main() -> Result<(), Box<dyn Error>> {
    let result = run_demo()?;

    println!(
        "Placed {} at ({}, {}) and {} at ({}, {})",
        result.gateway_name,
        result.gateway_position.x,
        result.gateway_position.y,
        result.service_name,
        result.service_position.x,
        result.service_position.y
    );
    for frame in result.frames {
        println!("\n{}", frame.view.trim_end());
    }

    Ok(())
}
