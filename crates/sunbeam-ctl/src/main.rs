use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "sunbeamctl")]
#[command(about = "CLI control tool for Sunbeam host")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Sessions,
    Select { agent_id: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Sessions => {
            println!("ID\tBACKEND\tNAME\tDISPLAY\tRESOLUTION");
            println!("x11-:0\tx11\tLocal Desktop\t:0\t2560x1440");
            println!("x11-:1\tx11\tMedia Desktop\t:1\t1920x1080");
        }
        Command::Select { agent_id } => {
            println!("Selected active session: {agent_id}");
        }
    }

    Ok(())
}
