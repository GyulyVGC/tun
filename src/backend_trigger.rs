use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use nullnet_grpc_lib::NullnetGrpcInterface;

pub const TRIGGER_PORT: u16 = 8888;

#[get("/trigger/{service_name}")]
async fn trigger(grpc: web::Data<NullnetGrpcInterface>, path: web::Path<String>) -> impl Responder {
    let service_name = path.into_inner();
    match grpc.backend_trigger(service_name.clone()).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => {
            eprintln!("backend trigger for '{service_name}' failed: {e}");
            HttpResponse::InternalServerError().body(e)
        }
    }
}

/// Runs the HTTP trigger server on a dedicated OS thread with its own actix
/// runtime, so it doesn't tangle with the tokio runtime driving the rest of
/// the client.
pub fn spawn(grpc: NullnetGrpcInterface) {
    std::thread::Builder::new()
        .name("backend-trigger".into())
        .spawn(move || {
            let sys = actix_web::rt::System::new();
            sys.block_on(async move {
                let grpc = web::Data::new(grpc);
                let server =
                    HttpServer::new(move || App::new().app_data(grpc.clone()).service(trigger))
                        .bind(("0.0.0.0", TRIGGER_PORT))
                        .expect("failed to bind backend trigger HTTP server");
                server
                    .run()
                    .await
                    .expect("backend trigger HTTP server crashed");
            });
        })
        .expect("failed to spawn backend-trigger thread");
}
