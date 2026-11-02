//! The HTTP surface against a real Autumn app with a real Arena.

use autumn_web::config::AutumnConfig;
use autumn_web::test::{TestApp, TestClient};

fn client() -> TestClient {
    let mut config = AutumnConfig {
        profile: Some("test".into()),
        ..AutumnConfig::default()
    };
    // The browser sends the token in a header; the CSRF layer is Autumn's own
    // and is exercised end to end by verify.sh, not here.
    config.security.csrf.enabled = false;
    TestApp::new()
        .config(config)
        .routes(trellis_app::all_routes!())
        .state_initializer(|state| {
            trellis_app::install(state, 2026).expect("the Arena plants");
        })
        .build()
}

fn json(r: &autumn_web::test::TestResponse) -> serde_json::Value {
    serde_json::from_str(&r.text()).expect("a JSON body")
}

#[tokio::test]
async fn every_page_serves() {
    let c = client();
    for (path, marker) in [
        ("/", "canvas id=\"wall\""),
        ("/herbarium", "Nothing pressed yet"),
        ("/shed", "canvas id=\"shed\""),
        ("/about", "What is proved"),
        ("/engine.js", "TrellisEngine"),
        ("/render.js", "TrellisRender"),
        ("/trellis.js", "data-page"),
    ] {
        c.get(path)
            .send()
            .await
            .assert_status(200)
            .assert_body_contains(marker);
    }
}

#[tokio::test]
async fn the_arena_starts_sprouting_with_six_gardeners() {
    let c = client();
    let s = json(&c.get("/_arena.json").send().await);
    assert_eq!(s["round"], 1);
    assert_eq!(s["phase"], "sprouting");
    assert_eq!(s["radius"], 16);
    assert_eq!(s["tick"], 0);
    let seats = s["seats"].as_array().unwrap();
    assert_eq!(seats.len(), 6);
    assert!(seats.iter().all(|x| x["kind"] == "gardener"));
    assert_eq!(s["vines"].as_array().unwrap().len(), 6);
}

#[tokio::test]
async fn joining_issues_a_ticket_and_steering_needs_a_seat() {
    let c = client();
    let r = c.post("/join").form("name=Mark&hue=8").send().await;
    r.assert_status(200);
    let j = json(&r);
    let ticket = j["ticket"].as_str().unwrap().to_string();
    assert_eq!(ticket.len(), 16);
    assert_eq!(j["players"], 1);
    // Not seated until the next round is planted.
    c.post("/steer")
        .form(&format!("ticket={ticket}&turn=left"))
        .send()
        .await
        .assert_status(422);
    // Unknown tickets, bad turns, bad tickets and empty names are refused.
    c.post("/steer")
        .form("ticket=0000000000000001&turn=left")
        .send()
        .await
        .assert_status(404);
    c.post("/steer")
        .form(&format!("ticket={ticket}&turn=backwards"))
        .send()
        .await
        .assert_status(400);
    c.post("/steer")
        .form("ticket=nothex&turn=left")
        .send()
        .await
        .assert_status(400);
    c.post("/join")
        .form("name=%20%20&hue=8")
        .send()
        .await
        .assert_status(422);
    // Leaving works once.
    c.post("/leave")
        .form(&format!("ticket={ticket}"))
        .send()
        .await
        .assert_status(200);
    c.post("/leave")
        .form(&format!("ticket={ticket}"))
        .send()
        .await
        .assert_status(404);
}

#[tokio::test]
async fn a_missing_pressing_is_not_found() {
    let c = client();
    c.get("/pressing/99").send().await.assert_status(404);
    c.get("/pressing/99/json").send().await.assert_status(404);
}
