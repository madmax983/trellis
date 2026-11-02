//! The HTTP surface.

use std::convert::Infallible;
use std::sync::Arc;

use autumn_web::error::AutumnError;
use autumn_web::prelude::*;
use autumn_web::sse::{Sse, from_subscriber};
use serde::{Deserialize, Serialize};
use tokio_stream::Stream;
use trellis::hex::Turn;

use crate::arena::{Arena, ArenaError, Pressed, RoundNo, Snapshot, TOPIC, Ticket};
use crate::view;

fn the_arena(state: &AppState) -> AutumnResult<Arc<Arena>> {
    crate::arena(state).ok_or_else(|| AutumnError::service_unavailable_msg("the Arena is planting"))
}

fn refuse(e: &ArenaError) -> AutumnError {
    match e {
        ArenaError::UnknownTicket => AutumnError::not_found_msg(e.to_string()),
        ArenaError::Round(_) => AutumnError::internal_server_error_msg(e.to_string()),
        _ => AutumnError::unprocessable_msg(e.to_string()),
    }
}

/// The Arena.
#[get("/")]
#[public]
pub async fn index(csrf: Option<CsrfToken>) -> Markup {
    view::arena_page(csrf.as_ref().map(CsrfToken::token))
}

/// The whole Arena, for a browser starting or catching up.
#[get("/_arena.json")]
#[public]
pub async fn arena_json(State(state): State<AppState>) -> AutumnResult<Json<Snapshot>> {
    Ok(Json(the_arena(&state)?.snapshot()))
}

/// A join, from a browser.
#[derive(Debug, Clone, Deserialize)]
pub struct JoinForm {
    /// Display name.
    pub name: String,
    /// Hue, 0–359.
    #[serde(default)]
    pub hue: u32,
}

/// The ticket a join earns.
#[derive(Debug, Clone, Serialize)]
pub struct JoinReply {
    /// Send it with every steer.
    pub ticket: String,
    /// People in the queue, including you.
    pub players: usize,
}

/// Join the queue for a seat in the next round.
#[post("/join")]
#[public]
pub async fn join(
    State(state): State<AppState>,
    Form(form): Form<JoinForm>,
) -> AutumnResult<Json<JoinReply>> {
    let a = the_arena(&state)?;
    let (ticket, event) = a.join(&form.name, form.hue).map_err(|e| refuse(&e))?;
    crate::publish(&state, &event);
    Ok(Json(JoinReply {
        ticket: ticket.hex(),
        players: a.snapshot().players,
    }))
}

/// A steer or a leave, from a browser.
#[derive(Debug, Clone, Deserialize)]
pub struct SteerForm {
    /// The ticket from `/join`.
    pub ticket: String,
    /// `left` or `right` (or `straight`, which queues a pause).
    #[serde(default)]
    pub turn: String,
}

/// What a steer did.
#[derive(Debug, Clone, Serialize)]
pub struct SteerReply {
    /// Your seat.
    pub vine: u8,
}

fn ticket(s: &str) -> AutumnResult<Ticket> {
    Ticket::parse(s).ok_or_else(|| AutumnError::bad_request_msg("a ticket is 16 hex digits"))
}

/// Queue a turn for your vine.
#[post("/steer")]
#[public]
pub async fn steer(
    State(state): State<AppState>,
    Form(form): Form<SteerForm>,
) -> AutumnResult<Json<SteerReply>> {
    let t = ticket(&form.ticket)?;
    let turn = Turn::parse(&form.turn)
        .ok_or_else(|| AutumnError::bad_request_msg("a turn is left, right or straight"))?;
    let v = the_arena(&state)?.steer(t, turn).map_err(|e| refuse(&e))?;
    Ok(Json(SteerReply { vine: v.0 }))
}

/// Give up your place.
#[post("/leave")]
#[public]
pub async fn leave(
    State(state): State<AppState>,
    Form(form): Form<SteerForm>,
) -> AutumnResult<Json<serde_json::Value>> {
    let t = ticket(&form.ticket)?;
    the_arena(&state)?.leave(t).map_err(|e| refuse(&e))?;
    Ok(Json(serde_json::json!({ "left": true })))
}

/// The live feed.
#[get("/stream")]
#[public]
pub async fn stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<autumn_web::sse::Event, Infallible>>> {
    from_subscriber(state.channels().subscribe(TOPIC))
}

/// The herbarium: every pressed round as a plate.
#[get("/herbarium")]
#[public]
pub async fn herbarium(State(state): State<AppState>) -> AutumnResult<Markup> {
    let pressed = the_arena(&state)?.herbarium();
    Ok(view::herbarium_page(&pressed))
}

/// One pressing, regrown from its seed and keystrokes.
#[get("/pressing/{round}")]
#[public]
pub async fn pressing(
    State(state): State<AppState>,
    Path(round): Path<u64>,
) -> AutumnResult<Markup> {
    let p = find(&state, round)?;
    let grown = p
        .pressing
        .regrow()
        .map_err(|e| AutumnError::internal_server_error_msg(e.to_string()))?;
    Ok(view::pressing_page(&p, &grown))
}

/// One pressing's causes, as JSON.
#[get("/pressing/{round}/json")]
#[public]
pub async fn pressing_json(
    State(state): State<AppState>,
    Path(round): Path<u64>,
) -> AutumnResult<Json<Pressed>> {
    Ok(Json(find(&state, round)?))
}

fn find(state: &AppState, round: u64) -> AutumnResult<Pressed> {
    the_arena(state)?
        .pressing(RoundNo(round))
        .ok_or_else(|| AutumnError::not_found_msg(format!("round {round} is not in the herbarium")))
}

/// The potting shed: the engine in the browser.
#[get("/shed")]
#[public]
pub async fn shed() -> Markup {
    view::shed_page()
}

/// The rules and the proofs.
#[get("/about")]
#[public]
pub async fn about() -> Markup {
    view::about_page()
}

fn script(body: &'static str) -> impl IntoResponse {
    (
        [
            ("content-type", "text/javascript; charset=utf-8"),
            ("cache-control", "no-cache"),
        ],
        body,
    )
}

/// The engine, ported.
#[get("/engine.js")]
#[public]
pub async fn engine_js() -> impl IntoResponse {
    script(view::ENGINE)
}

/// The renderer.
#[get("/render.js")]
#[public]
pub async fn render_js() -> impl IntoResponse {
    script(view::RENDER)
}

/// The page glue.
#[get("/trellis.js")]
#[public]
pub async fn trellis_js() -> impl IntoResponse {
    script(view::SCRIPT)
}

/// Every route, for `main` and the tests.
#[macro_export]
macro_rules! all_routes {
    () => {
        autumn_web::routes![
            $crate::routes::index,
            $crate::routes::arena_json,
            $crate::routes::join,
            $crate::routes::steer,
            $crate::routes::leave,
            $crate::routes::stream,
            $crate::routes::herbarium,
            $crate::routes::pressing,
            $crate::routes::pressing_json,
            $crate::routes::shed,
            $crate::routes::about,
            $crate::routes::engine_js,
            $crate::routes::render_js,
            $crate::routes::trellis_js,
        ]
    };
}
