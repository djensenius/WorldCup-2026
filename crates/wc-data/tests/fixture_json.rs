//! Fixture JSON smoke tests.

#[test]
fn backend_fixtures_are_valid_json() {
    for fixture in [
        "fixtures/espn_scoreboard.json",
        "fixtures/espn_standings.json",
        "fixtures/espn_summary.json",
        "fixtures/apifootball_scoreboard.json",
        "fixtures/apifootball_events.json",
        "fixtures/football_data_matches.json",
        "fixtures/football_data_standings.json",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(fixture);
        let text = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{fixture}"
        );
    }
}
