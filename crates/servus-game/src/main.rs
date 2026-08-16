use std::error::Error;

use servus_game::run_demo;

fn main() -> Result<(), Box<dyn Error>> {
    let result = run_demo()?;

    println!("Placed infrastructure:");
    for placement in &result.placements {
        println!(
            "- {} at ({}, {})",
            placement.name, placement.position.x, placement.position.y
        );
    }
    for frame in result.frames {
        println!("\n{}", frame.view.trim_end());
    }

    Ok(())
}
