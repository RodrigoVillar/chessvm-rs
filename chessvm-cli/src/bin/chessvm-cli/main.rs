use std::str::FromStr;

use alloy_primitives::Address;
use chessvm::{api::chain_handlers::MoveEnum, client};
use clap::{command, Arg, ArgMatches, Command};

#[tokio::main]
async fn main() {
    let matches =
        command!()
            .about("A CLI to interact with an existing ChessVM instance")
            .subcommand(
                Command::new("ping").about("Checks if the given instance of ChessVM is running"),
            )
            .subcommand(
                Command::new("does-game-exist")
                    .arg(
                        Arg::new("game-id")
                            .required(true)
                            .help("The game ID of the chess game"),
                    )
                    .about("Returns true if a game with the associated ID exists, false otherwise"),
            )
            .subcommand(
                Command::new("create-game")
                    .about("Creates a new Chess Game")
                    .arg(
                        Arg::new("white").required(true).help(
                            "The address of the white player; must be a valid Ethereum address.",
                        ),
                    )
                    .arg(Arg::new("black").required(true).help(
                        "The address of the black player; must be a valid Ethereum address.",
                    )),
            )
            .subcommand(
                Command::new("get-game")
                    .about("Returns FEN representation of the associated game if it exists")
                    .arg(Arg::new("game-id").help("The game ID of the chess game")),
            )
            .subcommand(
                Command::new("make-move")
                    .about("Creates a transaction for the move")
                    .subcommand(Command::new("normal").about(
                        "A regular chess move which is neither an En Passant nor Castling move",
                    )
                        .arg(
                            Arg::new("player-address").help("The Ethereum address of the player making the move").required(true)
                        )
                        .arg(
                            Arg::new("game-id").help("The ID of the game to perform the move on").required(true)
                        )
                        .arg(Arg::new("role").help("The type of piece you want to move; in FEN notation.").required(true))
                        .arg(Arg::new("from-square").help("The starting square of your piece").required(true))
                        .arg(Arg::new("to-square").help("The square where you want to move to").required(true))
                        .arg(Arg::new("capture-piece").help("The piece which you want to capture; in FEN notation"))
                        .arg(Arg::new("promotion-piece").help("The piece you want your pawn to promote to; in FEN notation"))
                    )
                    .subcommand(Command::new("en-passant").about("The En Passant chess move")
                        .arg(
                            Arg::new("player-address").help("The Ethereum address of the player making the move").required(true)
                        )
                        .arg(
                            Arg::new("game-id").help("The ID of the game to perform the move on").required(true)
                        )
                        .arg(Arg::new("from-square").help("The starting square of the pawn which you want to en passant with").required(true))
                        .arg(Arg::new("to-square").help("The square which you want your pawn to move to via en passant").required(true)))
                    .subcommand(
                        Command::new("castle")
                            .about("The castling move")
                            .arg(
                                Arg::new("player-address").help("The Ethereum address of the player making the move").required(true)
                            )
                            .arg(
                                Arg::new("game-id").help("The ID of the game to perform the move on").required(true)
                            )
                            .arg(
                                Arg::new("king-square")
                                    .help("The square of the king you are castling with")
                                    .required(true)
                            )
                            .arg(
                                Arg::new("rook-square")
                                    .help("The square of the rook you are castling with")
                                    .required(true)
                            )
                    ),
            )
            .arg(Arg::new("http-rpc").short('h').required(true))
            .arg(Arg::new("url-path").short('u').required(true))
            .get_matches();

    let http_rpc = matches
        .get_one::<String>("http-rpc")
        .expect("http-rpc is required!");
    let url_path = matches
        .get_one::<String>("url-path")
        .expect("url-path is required!");

    // println!("{}, {}", http_rpc, url_path);

    match matches.subcommand() {
        Some(("ping", _)) => execute_ping(http_rpc, url_path).await,
        Some(("does-game-exist", sub_args)) => {
            execute_does_game_exist(http_rpc, url_path, sub_args).await
        }
        Some(("create-game", sub_args)) => execute_create_game(http_rpc, url_path, sub_args).await,
        Some(("get-game", sub_args)) => execute_get_game(http_rpc, url_path, sub_args).await,
        Some(("make-move", sub_args)) => execute_make_move(http_rpc, url_path, sub_args).await,
        _ => panic!("Unknown subcommand!"),
    };
}

async fn execute_ping(http_rpc: &str, url_path: &str) {
    if let Ok(resp) = client::ping(http_rpc, url_path).await {
        if let Some(v) = resp.result {
            println!("Response is {}", v.success);
            return;
        }
    }
    println!("Ping failed!");
}
async fn execute_does_game_exist(http_rpc: &str, url_path: &str, sub_args: &ArgMatches) {
    let game_id = sub_args
        .get_one::<String>("game-id")
        .unwrap()
        .parse::<u64>()
        .unwrap();

    if let Ok(resp) = client::exists(http_rpc, url_path, game_id).await {
        println!("Response is {}", resp.result.unwrap().exists);
        return;
    }

    println!("Calling exist failed!");
}
async fn execute_create_game(http_rpc: &str, url_path: &str, sub_args: &ArgMatches) {
    // Parse out arguments
    let white = sub_args.get_one::<String>("white").unwrap().as_str();
    let white_addr = Address::from_str(white).unwrap();
    let black = sub_args.get_one::<String>("black").unwrap().as_str();
    let black_addr = Address::from_str(black).unwrap();

    if let Ok(resp) = client::create_game(http_rpc, url_path, white_addr, black_addr).await {
        println!(
            "Created Chess Game with ID: {}",
            resp.result.unwrap().game_id
        );
        return;
    }

    println!("Calling create_game failed!");
}
async fn execute_get_game(http_rpc: &str, url_path: &str, sub_args: &ArgMatches) {
    // Parse out arguments
    let game_id = sub_args
        .get_one::<String>("game-id")
        .unwrap()
        .parse::<u64>()
        .unwrap();

    if let Ok(resp) = client::get_game(http_rpc, url_path, game_id).await {
        // println!(
        //     "Current game board is the following: {}",
        //     resp.result.unwrap().game
        // );
        println!("Current game board is the following: ");
        print_chess_board_from_fen(&resp.result.unwrap().game);
        return;
    }

    println!("Failed to call get_game!");
}


fn print_chess_board_from_fen(fen: &String) {
    // Split the FEN string at spaces, and take the first part which represents the board
    let board_fen = fen.split_whitespace().next().unwrap();

    // Iterate over each character in the board representation
    for c in board_fen.chars() {
        match c {
            '1'..='8' => {
                // If the character is a digit, replace it with that many spaces
                let spaces = c.to_digit(10).unwrap() as usize;
                print!("{}", " ".repeat(spaces));
            }
            '/' => println!(), // If the character is a slash, move to the next line
            _ => print!("{}", c), // Otherwise, print the character as-is
        }
    }
    println!(); // Ensure the output ends with a newline
}

async fn execute_make_move(http_rpc: &str, url_path: &str, sub_args: &ArgMatches) {
    async fn execute_en_passant_move(http_rpc: &str, url_path: &str, sub_args: &ArgMatches) {
        // Extract args
        let player = Address::from_str(
            sub_args
                .get_one::<String>("player-address")
                .unwrap()
                .as_str(),
        )
        .unwrap();
        let game_id = sub_args
            .get_one::<String>("game-id")
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let from_square = sub_args
            .get_one::<String>("from-square")
            .unwrap()
            .to_owned();
        let to_square = sub_args
            .get_one::<String>("to-square")
            .unwrap()
            .to_owned();

        // Make call
        if let Ok(resp) = client::make_move(
            http_rpc,
            url_path,
            player,
            game_id,
            MoveEnum::EnPassant {
                from: from_square,
                to: to_square,
            },
        )
        .await
        {
            println!(
                "En Passant Transaction Submission Status: {}",
                resp.result.unwrap().status
            );
            return;
        }

        println!("Failed to submit En Passant Transaction!");
    }

    async fn execute_normal_move(http_rpc: &str, url_path: &str, sub_args: &ArgMatches) {
        // Extract args
        let player = Address::from_str(
            sub_args
                .get_one::<String>("player-address")
                .unwrap()
                .as_str(),
        )
        .unwrap();
        let game_id = sub_args
            .get_one::<String>("game-id")
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let role = sub_args.get_one::<String>("role").unwrap().to_owned();
        let from_square = sub_args
            .get_one::<String>("from-square")
            .unwrap()
            .to_owned();
        let to_square = sub_args.get_one::<String>("to-square").unwrap().to_owned();
        // Optional args
        let capture_piece = sub_args
            .get_one::<String>("capture-piece")
            .map(|x| x.to_owned());
        let promotion_piece = sub_args
            .get_one::<String>("promotion-piece")
            .map(|x| x.to_owned());

        // Make call
        if let Ok(resp) = client::make_move(
            http_rpc,
            url_path,
            player,
            game_id,
            MoveEnum::Normal {
                role,
                from: from_square,
                capture: capture_piece,
                to: to_square,
                promotion: promotion_piece,
            },
        )
        .await
        {
            println!(
                "Normal Move Transaction Submission Status: {}",
                resp.result.unwrap().status
            );
            return;
        }

        println!("Failed to make normal move transaction!");
    }

    async fn execute_castle_move(http_rpc: &str, url_path: &str, sub_args: &ArgMatches) {
        // Extract args
        let player = Address::from_str(
            sub_args
                .get_one::<String>("player-address")
                .unwrap()
                .as_str(),
        )
        .unwrap();
        let game_id = sub_args
            .get_one::<String>("game-id")
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let king_square = sub_args
            .get_one::<String>("king-square")
            .unwrap()
            .to_owned();
        let rook_square = sub_args
            .get_one::<String>("rook-square")
            .unwrap()
            .to_owned();

        // Make call
        if let Ok(resp) = client::make_move(
            http_rpc,
            url_path,
            player,
            game_id,
            MoveEnum::Castle {
                king: king_square,
                rook: rook_square,
            },
        )
        .await
        {
            println!(
                "Castling Transaction Submission Status: {}",
                resp.result.unwrap().status
            );
            return;
        }

        println!("Failed to make Castling Transaction!");
    }

    match sub_args.subcommand() {
        Some(("normal", ssub_args)) => execute_normal_move(http_rpc, url_path, ssub_args).await,
        Some(("en-passant", ssub_args)) => {
            execute_en_passant_move(http_rpc, url_path, ssub_args).await
        }
        Some(("castle", ssub_args)) => execute_castle_move(http_rpc, url_path, ssub_args).await,
        _ => panic!("not a valid move subcommand!"),
    }
}
