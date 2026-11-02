// Trellis — a garden wall where vines race to strangle each other.
//
// Run with:  cargo run --release -p trellis-app
// Then open: http://localhost:3000   (open it twice: the Arena is shared)

#[autumn_web::main]
async fn main() {
    Box::pin(
        autumn_web::app()
            .on_startup(trellis_app::boot)
            .routes(trellis_app::all_routes!())
            .run(),
    )
    .await;
}
