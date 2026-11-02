//! The shell every page is rendered into, the pages, and the herbarium plates.
//!
//! The Arena is a garden wall at night — loam, a faint lattice, vines in the
//! colour of whoever grows them. The herbarium is the morning after: the same
//! stems pressed flat on cream paper, as botanical plates.
//!
//! Autumn sets `script-src 'self'`, so every script is served from this app
//! and wired by `data-` attributes, never inline.

use std::fmt::Write as _;

use autumn_web::{Markup, PreEscaped, html};
use trellis::garden::{Outcome, Vine};
use trellis::gardener::Temper;
use trellis::hex::Board;
use trellis::round::Round;

use crate::arena::{CAST, Pressed, SeatView, epitaph};

/// The stylesheet.
pub const STYLE: &str = include_str!("trellis.css");
/// The engine port.
pub const ENGINE: &str = include_str!("engine.js");
/// The renderer.
pub const RENDER: &str = include_str!("render.js");
/// The page glue.
pub const SCRIPT: &str = include_str!("trellis.js");

fn page(title: &str, here: &str, csrf: Option<&str>, data_page: &str, content: &Markup) -> Markup {
    let nav = [
        ("/", "The Arena"),
        ("/herbarium", "Herbarium"),
        ("/shed", "Potting shed"),
        ("/about", "How it grows"),
    ];
    html! {
        (PreEscaped("<!DOCTYPE html>"))
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " — Trellis" }
                @if let Some(t) = csrf { meta name="csrf-token" content=(t); }
                style { (PreEscaped(STYLE)) }
                script src="/engine.js" defer {}
                script src="/render.js" defer {}
                script src="/trellis.js" defer {}
            }
            body data-page=(data_page) {
                div class="wrap" {
                    header class="top" {
                        h1 { a href="/" { "Trellis" } }
                        span class="tag" { "a garden wall where vines race to strangle each other" }
                        nav {
                            @for (href, label) in nav {
                                a href=(href) class=(if href == here { "here" } else { "" }) { (label) }
                            }
                        }
                    }
                    (content)
                    footer {
                        "Every tick every vine grows one cell — straight on, or sixty degrees left or right. "
                        "Grow off the wall, into any stem, or into the same cell as another vine, and you wilt. "
                        "The last vine growing wins the wall."
                    }
                }
            }
        }
    }
}

const HUES: [u32; 8] = [8, 28, 52, 160, 190, 215, 300, 340];

/// The Arena.
pub fn arena_page(csrf: Option<&str>) -> Markup {
    let content = html! {
        main class="arena" {
            section class="wallbox" {
                div class="frame" {
                    canvas id="wall" width="900" height="820" {}
                    div class="banner" id="banner" {}
                    div class="pads" {
                        button class="pad" data-turn="left" aria-label="Turn left" { "⟲" }
                        button class="pad" data-turn="right" aria-label="Turn right" { "⟳" }
                    }
                }
                div class="under" {
                    span id="roundinfo" { "planting…" }
                    span class="spacer" {}
                    span class="live" id="live" { "●" }
                }
            }
            aside class="panel" {
                h2 { "On the wall" }
                ol class="cast" id="cast" {}
                h2 { "Take a vine" }
                div class="join" id="joinbox" {
                    p class="hint" { "Join and you get the next free seat. Steer with "
                        kbd { "←" } " " kbd { "→" } " (or " kbd { "A" } " " kbd { "D" } ", or the pads on a phone)." }
                    div class="me" {
                        input id="name" maxlength="16" placeholder="your name" autocomplete="off";
                        button id="join" class="go" { "Join" }
                    }
                    div class="hues" id="hues" {
                        @for h in HUES {
                            button class="hue" data-hue=(h) style=(format!("--h:{h}")) aria-label=(format!("hue {h}")) {}
                        }
                    }
                    p class="hint" id="joinstate" {}
                }
                h2 { "The wall remembers" }
                ul class="log" id="log" {}
                h2 { "Wins since dawn" }
                table class="wins" id="wins" {}
            }
        }
    };
    page("The Arena", "/", csrf, "arena", &content)
}

fn hsl(hue: u32, light: u32) -> String {
    format!("hsl({hue} 62% {light}%)")
}

/// Pointy-top hex centre for axial `(q, r)` at `size`, offset to `(cx, cy)`.
fn centre(q: i32, r: i32, size: f64, cx: f64, cy: f64) -> (f64, f64) {
    let s3 = 3.0_f64.sqrt();
    (
        f64::from(q)
            .mul_add(s3, f64::from(r) * s3 / 2.0)
            .mul_add(size, cx),
        (f64::from(r) * 1.5).mul_add(size, cy),
    )
}

/// A pressed round as an SVG botanical plate.
pub fn plate(
    board: &Board,
    vines: &[Vine],
    seats: &[SeatView],
    outcome: Outcome,
    px: u32,
) -> Markup {
    let radius = f64::from(board.radius());
    let size = f64::from(px) / radius.mul_add(2.0, 1.6) / 3.0_f64.sqrt();
    let half = f64::from(px) / 2.0;
    let mut svg = String::new();
    let _ = write!(
        svg,
        r#"<svg class="plate" viewBox="0 0 {px} {px}" xmlns="http://www.w3.org/2000/svg" role="img">"#
    );
    // The trellis: faint dots at every cell.
    let _ = write!(svg, r##"<g fill="#b9a982" opacity="0.45">"##);
    for c in board.cells() {
        let (q, rr) = board.coords(c);
        let (x, y) = centre(q, rr, size, half, half);
        let _ = write!(
            svg,
            r#"<circle cx="{x:.1}" cy="{y:.1}" r="{:.2}"/>"#,
            size * 0.12
        );
    }
    svg.push_str("</g>");
    for (k, v) in vines.iter().enumerate() {
        let hue = seats.get(k).map_or(0, |s| s.hue);
        let pts: Vec<String> = v
            .stem
            .iter()
            .map(|c| {
                let (q, rr) = board.coords(*c);
                let (x, y) = centre(q, rr, size, half, half);
                format!("{x:.1},{y:.1}")
            })
            .collect();
        let _ = write!(
            svg,
            r#"<polyline class="stem" data-cells="{}" points="{}" fill="none" stroke="{}" stroke-width="{:.2}" stroke-linecap="round" stroke-linejoin="round"/>"#,
            v.stem.len(),
            pts.join(" "),
            hsl(hue, 38),
            size * 0.55
        );
        // Leaves, alternating sides, every third cell.
        for (i, w) in v.stem.windows(2).enumerate() {
            if i % 3 != 1 {
                continue;
            }
            let (q0, r0) = board.coords(w[0]);
            let (q1, r1) = board.coords(w[1]);
            let (x0, y0) = centre(q0, r0, size, half, half);
            let (x1, y1) = centre(q1, r1, size, half, half);
            let ang = (y1 - y0).atan2(x1 - x0).to_degrees() + if i % 2 == 0 { 55.0 } else { -55.0 };
            let _ = write!(
                svg,
                r#"<ellipse cx="{:.1}" cy="{:.1}" rx="{:.2}" ry="{:.2}" transform="rotate({ang:.0} {:.1} {:.1}) translate({:.2} 0)" fill="{}" opacity="0.8"/>"#,
                f64::midpoint(x0, x1),
                f64::midpoint(y0, y1),
                size * 0.42,
                size * 0.2,
                f64::midpoint(x0, x1),
                f64::midpoint(y0, y1),
                size * 0.38,
                hsl(hue, 48)
            );
        }
        // A bloom where it ended.
        let (q, rr) = board.coords(v.head);
        let (x, y) = centre(q, rr, size, half, half);
        let won = outcome
            == (Outcome::Won {
                by: trellis::garden::VineId(u8::try_from(k).unwrap_or(0)),
            });
        let petal = if won { size * 0.95 } else { size * 0.55 };
        for p in 0..5 {
            let a = f64::from(p) * 72.0;
            let _ = write!(
                svg,
                r#"<ellipse cx="{x:.1}" cy="{:.1}" rx="{:.2}" ry="{:.2}" transform="rotate({a:.0} {x:.1} {y:.1})" fill="{}"/>"#,
                y - petal * 0.6,
                petal * 0.42,
                petal * 0.62,
                hsl(hue, 66)
            );
        }
        let _ = write!(
            svg,
            r##"<circle cx="{x:.1}" cy="{y:.1}" r="{:.2}" fill="#f4d35e"/>"##,
            petal * 0.28
        );
    }
    svg.push_str("</svg>");
    PreEscaped(svg)
}

fn caption(p: &Pressed) -> String {
    match p.outcome {
        Outcome::Won { by } => {
            let name = p.seats.get(by.ix()).map_or("?", |s| s.name.as_str());
            format!("{name} won in {} ticks", p.pressing.ticks)
        }
        Outcome::Draw => format!("a draw in {} ticks", p.pressing.ticks),
        Outcome::Growing => format!("interrupted at tick {}", p.pressing.ticks),
    }
}

/// The herbarium.
pub fn herbarium_page(pressed: &[Pressed]) -> Markup {
    let content = html! {
        main class="herbarium" {
            p class="lede" {
                "Every finished round, pressed flat. A pressing is stored as its seed and the "
                "humans' keystrokes — nothing else — and each plate below was regrown from those "
                "just now, stem for stem."
            }
            @if pressed.is_empty() {
                p class="hint" { "Nothing pressed yet. The first round is still growing on the wall." }
            }
            div class="plates" {
                @for p in pressed {
                    @if let Ok(round) = p.pressing.regrow() {
                        a class="sheet" href=(format!("/pressing/{}", p.round.0)) {
                            (plate(round.garden().board(), round.garden().vines(), &p.seats, p.outcome, 300))
                            span class="label" {
                                span class="no" { "No. " (p.round.0) }
                                " " (caption(p))
                            }
                        }
                    }
                }
            }
        }
    };
    page("Herbarium", "/herbarium", None, "herbarium", &content)
}

/// One pressing.
pub fn pressing_page(p: &Pressed, grown: &Round) -> Markup {
    let g = grown.garden();
    let content = html! {
        main class="pressing" {
            div class="bigsheet" {
                (plate(g.board(), g.vines(), &p.seats, p.outcome, 720))
                span class="label" { span class="no" { "No. " (p.round.0) } " " (caption(p)) }
            }
            aside class="panel" {
                h2 { "The cast" }
                table class="castlist" {
                    @for (k, v) in g.vines().iter().enumerate() {
                        @let s = &p.seats[k];
                        tr {
                            td { span class="dot" style=(format!("background:{}", hsl(s.hue, 58))) {} }
                            td class="who" { (s.name) @if let Some(t) = s.temper { span class="temper" { " " (t.name()) } } @else { span class="temper" { " person" } } }
                            td class="num" { (v.stem.len()) }
                        }
                        tr class="ep" {
                            td {}
                            td colspan="2" {
                                @match v.wilted {
                                    Some((tick, d)) => { "tick " (tick) ": " (epitaph(&p.seats, trellis::garden::VineId(u8::try_from(k).unwrap_or(0)), d)) }
                                    None => { "still growing when the wall was won" }
                                }
                            }
                        }
                    }
                }
                h2 { "Its causes" }
                p class="hint" {
                    "seed " code { (format!("{:#010x}", p.pressing.seed)) } ", radius " (p.pressing.radius)
                    ", " (p.pressing.keys.len()) " human keystrokes. "
                    "That is the whole record: the gardeners are deterministic, so the plate is regrown from it."
                }
                p { button class="go" id="regrow" { "Watch it regrow" } }
                p class="hint" { a href=(format!("/pressing/{}/json", p.round.0)) { "the pressing as JSON" } " · " a href="/herbarium" { "back to the herbarium" } }
            }
        }
    };
    page(
        &format!("Pressing No. {}", p.round.0),
        "/herbarium",
        None,
        "pressing",
        &content,
    )
}

/// The potting shed.
pub fn shed_page() -> Markup {
    let choices = [
        ("you", "You"),
        ("ivy", "Ivy"),
        ("kudzu", "Kudzu"),
        ("moss", "Moss"),
        ("hops", "Hops"),
        ("none", "—"),
    ];
    let defaults = ["you", "ivy", "kudzu", "moss", "hops", "ivy"];
    let content = html! {
        main class="arena" {
            section class="wallbox" {
                div class="frame" {
                    canvas id="shed" width="900" height="820" {}
                    div class="banner" id="banner" {}
                    div class="pads" {
                        button class="pad" data-turn="left" aria-label="Turn left" { "⟲" }
                        button class="pad" data-turn="right" aria-label="Turn right" { "⟳" }
                    }
                }
                div class="under" {
                    button class="go" id="plant" { "Plant" }
                    button class="quiet" id="pause" { "Pause" }
                    button class="quiet" id="stepbtn" { "Step" }
                    label class="speed" { "speed " input type="range" id="speed" min="1" max="40" value="8"; }
                    span class="spacer" {}
                    span id="shedinfo" {}
                }
            }
            aside class="panel" {
                h2 { "Seats" }
                div class="seats" id="seats" {
                    @for (k, d) in defaults.iter().enumerate() {
                        label class="seat" {
                            span class="dot" data-seat=(k) {}
                            select data-seat=(k) {
                                @for (v, l) in choices {
                                    option value=(v) selected[v == *d] { (l) }
                                }
                            }
                        }
                    }
                }
                div class="me" {
                    input id="word" maxlength="40" placeholder="a word to seed the planting" value="trellis";
                }
                label class="speed" { "trellis radius " input type="range" id="radius" min="6" max="24" value="14"; span id="rlabel" { "14" } }
                h2 { "The gardener's eye" }
                p class="hint" { "Click a vine to see what its gardener sees: the territory it would own for each turn, and the score its temper gives." }
                label class="check" { input type="checkbox" id="eye" checked; " shade territory" }
                table class="looks" id="looks" {}
                h2 { "Tempers" }
                ul class="tempers" {
                    @for t in Temper::ALL {
                        li { b { (t.name()) } " — " (t.motto()) }
                    }
                }
            }
        }
    };
    page("Potting shed", "/shed", None, "shed", &content)
}

/// How it works.
pub fn about_page() -> Markup {
    let content = html! {
        main class="about" {
            h2 { "The rules" }
            p { "The wall is a hexagon of hexagons: radius 16, 817 cells. Six vines are planted on the ring two thirds of the way out. On every tick, at the same instant, every growing vine:" }
            ol {
                li { "turns its heading by its turn — straight on, sixty degrees left or sixty degrees right (never back);" }
                li { "reaches for the neighbouring cell across the new heading;" }
                li { "wilts if that cell is off the wall, already holds any stem — its own included — or is the cell another vine reached for this tick;" }
                li { "otherwise takes it." }
            }
            p { "The last vine growing wins. If the last vines wilt together, it is a draw." }
            h2 { "The gardeners" }
            p { "Every empty seat is a gardener. Each looks one move ahead: for each turn that does not wilt at once it floods the wall to count " em { "reach" } " (how much room it would have), splits the wall into Voronoi " em { "territory" } " (cells strictly nearer its head than any rival's) and checks for " em { "contact" } " (whether any rival can still reach it). Sealed off from everyone, every gardener stops fighting and starts packing: as much reach as possible, hugging walls. Otherwise each scores by temper:" }
            ul {
                @for (name, t, hue) in CAST {
                    li { span class="dot" style=(format!("background:{}", hsl(hue, 58))) {} " " b { (name) } " (" (t.name()) ") — " (t.motto()) }
                }
            }
            p { "Everything is integer arithmetic over a fixed visit order, so the browser port in the potting shed chooses exactly what the server chooses — checked stem for stem by " code { "tools/parity.js" } "." }
            h2 { "What is proved" }
            p { "In " code { "verus/src/trellis.rs" } ", against a pure model of one tick:" }
            ol {
                li { b { "No cell holds two stems." } " Survivors' targets are pairwise distinct and empty, so the step never overwrites a stem and never writes one cell twice." }
                li { b { "Stems are forever." } " Every cell held before a tick is held by the same vine after it." }
                li { b { "The wall fills." } " A tick in which " em { "s" } " vines survive fills exactly " em { "s" } " cells; so while anything grows, the number of empty cells strictly falls, and every round ends within as many ticks as the wall has cells." }
                li { b { "No vine reverses." } " A turned heading is never the opposite of the old one, so no vine can grow back into its own neck by choosing to." }
                li { b { "The trellis is symmetric." } " Stepping across a heading and back across its opposite returns to the start, and stays within radius." }
            }
            p { "Property tests in " code { "crates/trellis/tests/laws.rs" } " check the same laws against the running engine, and the Arena's simulation test drives hundreds of rounds on Autumn's virtual clock." }
        }
    };
    page("How it grows", "/about", None, "about", &content)
}
