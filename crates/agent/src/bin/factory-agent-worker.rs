//! Standalone worker process: `factory-agent-worker --generation N`.
//!
//! The server also exposes the same loop through its own `agent-worker`
//! subcommand; both call `factory_agent::worker::serve`.

fn main() {
    let mut args = std::env::args().skip(1);
    let mut generation: Option<u64> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--generation" => generation = args.next().and_then(|v| v.parse().ok()),
            other => {
                eprintln!("factory-agent-worker: unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let Some(generation) = generation else {
        eprintln!("usage: factory-agent-worker --generation N");
        std::process::exit(2);
    };
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("factory-agent-worker: runtime: {e}");
            std::process::exit(1);
        }
    };
    let code = match rt.block_on(factory_agent::worker::serve(
        tokio::io::stdin(),
        tokio::io::stdout(),
        generation,
    )) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("factory-agent-worker: {e}");
            1
        }
    };
    // A blocked stdin read would otherwise keep the runtime alive.
    std::process::exit(code);
}
