extern crate irc;
extern crate debug;
extern crate readline;

use std::str::MaybeOwned;
use irc::IrcConnection;


const COMMAND_PREFIX: &'static str = ".";

const PROMPT_DESIRED_NICK: &'static str =
    "Please enter your desired nickname: ";

const PROMPT_CONNECTED: &'static str =
    "[connected] >>> ";

const PROMPT_DISCONNECTED: &'static str = 
    "[disconnected] !!! ";

trait CmdDescriptor {
    fn name() -> &'static str;
}

struct CmdNames;
impl CmdDescriptor for CmdNames {
    fn name() -> &'static str {
        "names"
    }
}

struct CmdJoin;
impl CmdDescriptor for CmdJoin {
    fn name() -> &'static str {
        "join"
    }
}

struct CmdSwitchChannel;
impl CmdDescriptor for CmdSwitchChannel {
    fn name() -> &'static str {
        "swch"
    }
}

enum ConnectionPhase {
    Registration,
    Connected,
    Disconnected
}

struct UserInterface<'a> {
    connection: &'a mut IrcConnection,
    current_phase: ConnectionPhase,
    command_desciptors: Vec<Box<CmdDescriptor+'static>>,
    current_channel: Option<String>,
}

impl<'a> UserInterface<'a> {
    fn new<'a>(conn: &'a mut IrcConnection) -> UserInterface<'a> {
        UserInterface {
            connection: conn,
            current_phase: Registration,
            command_desciptors: Vec::new(),
            current_channel: None
        }
    }

    fn parse_command<'a>(line: &'a str) -> Option<&'a str> {
        if line.starts_with(COMMAND_PREFIX) {
            Some(match line.find(' ') {
                Some(idx) => line[COMMAND_PREFIX.len()..idx],
                None => line[COMMAND_PREFIX.len()..]
            })
        } else {
            None
        }
    }

    fn get_current_prompt(&mut self) -> MaybeOwned<'static> {
        match self.current_phase {
            Registration => PROMPT_DESIRED_NICK.into_maybe_owned(),
            Connected => {
                PROMPT_CONNECTED.into_maybe_owned()
            },
            Disconnected => {
                PROMPT_DISCONNECTED.into_maybe_owned()
            }
        }
    }

    fn run_interface_registration(&mut self) {
        let nick = match readline::readline(PROMPT_DESIRED_NICK) {
            Some(nick) => nick,
            None => {
                println!("nick-read failed, exit")
                return;
            }
        };

        match self.connection.register(nick[].trim_chars('\n')) {
            Ok(_) => {
                self.current_phase = Connected;
            }
            Err(err) => println!("registration error: {:?}", err)
        }
    }

    fn run_interface_connected(&mut self) {
        let prompt = self.get_current_prompt();
        let line = match readline::readline(prompt.as_slice()) {
            Some(line) => line,
            None => return
        };
        let line_cleaned = line[].trim_chars('\n');
        match UserInterface::parse_command(line_cleaned) {
            Some(command) => {
                println!("got command: {}", command);
            },
            None => {
                self.connection.write_str(line_cleaned);
            }
        }   
    }

    fn run_interface_disconnected(&mut self) {
        let prompt = self.get_current_prompt();
        let line = match readline::readline(prompt.as_slice()) {
            Some(line) => line,
            None => return
        };
        let line_cleaned = line[].trim_chars('\n');
        if "quit" == line_cleaned {
            fail!("quitting");
        }
    }

    fn run_interface(&mut self) {
        loop {
            match self.current_phase {
                Registration => self.run_interface_registration(),
                Connected => self.run_interface_connected(),
                Disconnected => self.run_interface_disconnected(),
            };
        }
    }
}


fn main() {
    let (mut conn, eventstream) = match IrcConnection::new("127.0.0.1", 6667) {
        Ok(stuff) => stuff,
        Err(err) => {
            println!("Failed to connect: {}", err);
            // os::set_return_code or whatever
            return;
        }
    };

    spawn(proc() {
        for event in eventstream.iter() {
            println!("RX: {:?}", event);
        }
        
    });

    let mut ui = UserInterface::new(&mut conn);
    ui.run_interface();   
}
