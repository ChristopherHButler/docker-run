mod docker_run;

use std::process;
use std::time::Duration;

use docker_run::config;
use docker_run::environment;
use docker_run::unix_stream;
use docker_run::run;
use docker_run::api;


fn main() {
    env_logger::init();

    match start() {
        Ok(()) => {}

        Err(Error::BuildConfig(err)) => {
            log::error!("Failed to build config: {}", err);
            process::exit(1)
        }

        Err(Error::StartServer(err)) => {
            log::error!("Failed to start api server: {}", err);
            process::exit(1)
        }
    }
}

enum Error {
    BuildConfig(environment::Error),
    StartServer(api::Error),
}

fn start() -> Result<(), Error> {
    let env = environment::get_environment();
    let config = build_config(&env)
        .map_err(Error::BuildConfig)?;

    log::info!("Listening on {} with {} worker threads", config.server.listen_addr_with_port(), config.server.worker_threads);

    api::start(api::Config{
        listen_addr: config.server.listen_addr_with_port(),
        worker_threads: config.server.worker_threads,
        handler_config: config,
        handler: handle_request,
    }).map_err(Error::StartServer)
}


fn handle_request(config: &config::Config, mut request: tiny_http::Request) {

    let handler = router(&request);

    let result = match handler(&config, &mut request) {
        Ok(data) => {
            api::success_response(request, &data)
        }

        Err(err) => {
            api::error_response(request, err)
        }
    };

    match result {
        Ok(()) => {},

        Err(err) => {
            log::error!("Failure while sending response: {}", err)
        }
    }
}

fn router(request: &tiny_http::Request) -> fn(&config::Config, &mut tiny_http::Request) -> Result<Vec<u8>, api::ErrorResponse> {
    match (request.method(), request.url()) {
        (tiny_http::Method::Get, "/") => {
            api::root::handle
        }

        (tiny_http::Method::Post, "/run") => {
            api::run::handle
        }

        (tiny_http::Method::Get, "/version") => {
            api::version::handle
        }

        _ => {
            api::not_found::handle
        }
    }
}


fn build_config(env: &environment::Environment) -> Result<config::Config, environment::Error> {
    let server = build_server_config(env)?;
    let api = build_api_config(env)?;
    let unix_socket = build_unix_socket_config(env)?;
    let container = build_container_config(env)?;
    let run = build_run_config(env)?;

    Ok(config::Config{
        server,
        api,
        unix_socket,
        container,
        run,
    })
}

fn build_server_config(env: &environment::Environment) -> Result<config::ServerConfig, environment::Error> {
    let listen_addr = environment::lookup(env, "SERVER_LISTEN_ADDR")?;
    let listen_port = environment::lookup(env, "SERVER_LISTEN_PORT")?;
    let worker_threads = environment::lookup(env, "SERVER_WORKER_THREADS")?;

    Ok(config::ServerConfig{
        listen_addr,
        listen_port,
        worker_threads,
    })
}

fn build_api_config(env: &environment::Environment) -> Result<api::ApiConfig, environment::Error> {
    let access_token = environment::lookup(env, "API_ACCESS_TOKEN")?;

    Ok(api::ApiConfig{
        access_token,
    })
}

fn build_unix_socket_config(env: &environment::Environment) -> Result<unix_stream::Config, environment::Error> {
    let path = environment::lookup(env, "DOCKER_UNIX_SOCKET_PATH")?;
    let read_timeout = environment::lookup(env, "DOCKER_UNIX_SOCKET_READ_TIMEOUT")?;
    let write_timeout = environment::lookup(env, "DOCKER_UNIX_SOCKET_WRITE_TIMEOUT")?;

    Ok(unix_stream::Config{
        path,
        read_timeout: Duration::from_secs(read_timeout),
        write_timeout: Duration::from_secs(write_timeout),
    })
}

fn build_container_config(env: &environment::Environment) -> Result<run::ContainerConfig, environment::Error> {
    let hostname = environment::lookup(env, "DOCKER_CONTAINER_HOSTNAME")?;
    let user = environment::lookup(env, "DOCKER_CONTAINER_USER")?;
    let memory = environment::lookup(env, "DOCKER_CONTAINER_MEMORY")?;
    let ulimit_nofile_soft = environment::lookup(env, "DOCKER_CONTAINER_ULIMIT_NOFILE_SOFT")?;
    let ulimit_nofile_hard = environment::lookup(env, "DOCKER_CONTAINER_ULIMIT_NOFILE_HARD")?;
    let ulimit_nproc_soft = environment::lookup(env, "DOCKER_CONTAINER_ULIMIT_NPROC_SOFT")?;
    let ulimit_nproc_hard = environment::lookup(env, "DOCKER_CONTAINER_ULIMIT_NPROC_HARD")?;
    let cap_add = environment::lookup(env, "DOCKER_CONTAINER_CAP_ADD").unwrap_or_default();
    let cap_drop = environment::lookup(env, "DOCKER_CONTAINER_CAP_DROP").unwrap_or_default();

    Ok(run::ContainerConfig{
        hostname,
        user,
        memory,
        ulimit_nofile_soft,
        ulimit_nofile_hard,
        ulimit_nproc_soft,
        ulimit_nproc_hard,
        cap_add: environment::space_separated_string(cap_add),
        cap_drop: environment::space_separated_string(cap_drop),
    })
}

fn build_run_config(env: &environment::Environment) -> Result<run::Limits, environment::Error> {
    let max_execution_time = environment::lookup(env, "RUN_MAX_EXECUTION_TIME")?;
    let max_output_size = environment::lookup(env, "RUN_MAX_OUTPUT_SIZE")?;

    Ok(run::Limits{
        max_execution_time: Duration::from_secs(max_execution_time),
        max_output_size,
    })
}
