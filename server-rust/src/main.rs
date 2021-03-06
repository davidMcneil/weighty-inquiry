#![feature(proc_macro_hygiene, decl_macro)]

mod question_lookup;
#[cfg(test)]
mod tests;
mod types;

use parking_lot::Mutex;
use question_lookup::QuestionLookup;
use rocket::{
    self,
    config::{Environment, LoggingLevel},
    delete, get, post, put, routes, Config, State,
};
use rocket_contrib::json::Json;
use std::path::PathBuf;
use structopt::StructOpt;
use types::{Answer, Game, Guess, PlayerData, Result};

type Games = Mutex<types::Games>;
type Questions = Mutex<QuestionLookup>;

#[get("/")]
fn index() -> &'static str {
    // TODO: This will eventually return the client
    "html"
}

#[get("/heartbeat")]
fn heartbeat() -> &'static str {
    "heartbeat"
}

#[put("/game/<game_id>", data = "<player>")]
fn create_game(
    game_id: String,
    player: Json<PlayerData>,
    games: State<Games>,
    questions: State<Questions>,
) -> Result<()> {
    let mut games = games.lock();
    let player = player.into_inner();
    games.create(game_id, player.player, questions.lock().get())
}

#[post("/game/<game_id>", data = "<player>")]
fn join_game(game_id: String, player: Json<PlayerData>, games: State<Games>) -> Result<()> {
    let mut games = games.lock();
    let game = games.get(&game_id)?;
    let player = player.into_inner();
    game.add_player(player.player)
}

#[get("/game/<game_id>")]
fn game(game_id: String, games: State<Games>) -> Result<Json<Game>> {
    let mut games = games.lock();
    let game = games.get(&game_id)?;
    // TODO: This clone is ugly
    Ok(Json(game.clone()))
}

#[post("/game/<game_id>/answer", data = "<answer>")]
fn answer(game_id: String, answer: Json<Answer>, games: State<Games>) -> Result<()> {
    let mut games = games.lock();
    let game = games.get(&game_id)?;
    let answer = answer.into_inner();
    game.answer(answer)
}

#[post("/game/<game_id>/guess", data = "<guess>")]
fn guess(
    game_id: String,
    guess: Json<Guess>,
    games: State<Games>,
    questions: State<Questions>,
) -> Result<()> {
    let mut games = games.lock();
    let game = games.get(&game_id)?;
    let guess = guess.into_inner();
    game.guess(guess)?;
    game.add_round_if_complete(questions.lock().get());
    Ok(())
}

#[delete("/game/<game_id>/exit", data = "<player>")]
fn exit_game(game_id: String, player: Json<PlayerData>, games: State<Games>) -> Result<()> {
    let mut games = games.lock();
    let game = games.get(&game_id)?;
    let player = player.into_inner();
    game.remove_player(player.player)
}

#[delete("/game/<game_id>")]
fn delete_game(game_id: String, games: State<Games>) {
    let mut games = games.lock();
    games.delete(&game_id)
}

fn rocket(opt: Option<Opt>) -> rocket::Rocket {
    let mut questions = QuestionLookup::default();
    let rocket = if let Some(opt) = opt {
        if let Some(questions_file) = opt.questions_file {
            // Populate the questions
            if let Err(e) = questions.populate_from_file(&questions_file) {
                eprintln!(
                    "Failed to populate questions from file {:?}, err: {}",
                    questions_file, e
                );
                std::process::exit(1);
            }
        }
        // Configure the Rocket instance
        let config = Config::build(Environment::Staging)
            .address(opt.host)
            .port(opt.port)
            .log_level(opt.log_level)
            .finalize()
            .expect("to build Rocket Config");
        rocket::custom(config)
    } else {
        rocket::ignite()
    };
    rocket
        .mount("/", routes![index])
        .mount(
            "/api/v1",
            routes![
                heartbeat,
                create_game,
                join_game,
                game,
                answer,
                guess,
                exit_game,
                delete_game
            ],
        )
        .manage(Mutex::new(questions))
        .manage(Games::default())
}

#[derive(Debug, StructOpt)]
struct Opt {
    /// The path to a file containing newline delimited questions.
    #[structopt(long = "questions-file")]
    questions_file: Option<PathBuf>,
    /// An IP address or host the application will listen on.
    #[structopt(long = "host", short = "H", default_value = "0.0.0.0")]
    host: String,
    /// A port number to listen on.
    #[structopt(long = "port", short = "P", default_value = "8172")]
    port: u16,
    /// The log level.
    #[structopt(
        default_value = "normal",
        long = "log-level",
        possible_values = &["off", "debug", "normal", "critical"]
    )]
    log_level: LoggingLevel,
}

fn main() {
    let opt = Opt::from_args();
    rocket(Some(opt)).launch();
}
