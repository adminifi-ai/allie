const PRODUCT_LINE: &str = "Allie: accessibility evidence for every release.";
const NEXT_STEP: &str = "Next implementation target: allie run --manifest <flow.yml>";

fn product_line() -> &'static str {
    PRODUCT_LINE
}

fn next_step() -> &'static str {
    NEXT_STEP
}

fn main() {
    println!("{}", product_line());
    println!("{}", next_step());
    println!("Read SPEC.md for the product contract and docs/roadmap.md for the first slice.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_line_names_the_release_evidence_goal() {
        assert!(product_line().contains("accessibility evidence"));
        assert!(product_line().contains("every release"));
    }

    #[test]
    fn next_step_points_to_the_first_cli_surface() {
        assert!(next_step().contains("allie run"));
        assert!(next_step().contains("--manifest"));
    }
}
