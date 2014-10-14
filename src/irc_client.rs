#![feature(if_let)]
#![feature(slicing_syntax)] 

extern crate irc;
extern crate debug;
extern crate readline;

use std::comm::sync_channel;
use std::str::SendStr;
use irc::{IrcConnection};
use irc::watchers::event::{
    IrcEvent,
    IrcEventMessage
};


const COMMAND_PREFIX: &'static str = ".";

const PROMPT_DESIRED_NICK: &'static str =
    "Please enter your desired nickname: ";

const PROMPT_CONNECTED: &'static str =
    "[connected] >>> ";

const PROMPT_DISCONNECTED: &'static str = 
    "[disconnected] !!! ";


enum UiCommand {
    UpdatePrompt(SendStr),
    PrintLn(String)
}


type BoxedCmdDescriptor = Box<CmdDescriptor+'static>;

trait CmdDescriptor {
    fn name(&self) -> &'static str;
}


struct CmdNames;

impl CmdNames {
    fn new() -> CmdNames {
        CmdNames
    }

    fn create() -> BoxedCmdDescriptor {
        box CmdNames::new() as BoxedCmdDescriptor
    }
}

impl CmdDescriptor for CmdNames {
    fn name(&self) -> &'static str {
        "names"
    }
}

struct CmdJoin;

impl CmdJoin {
    fn new() -> CmdJoin {
        CmdJoin
    }

    fn create() -> BoxedCmdDescriptor {
        box CmdJoin::new() as BoxedCmdDescriptor
    }
}

impl CmdDescriptor for CmdJoin {
    fn name(&self) -> &'static str {
        "join"
    }
}

struct CmdSwitchChannel;

impl CmdSwitchChannel {
    fn new() -> CmdSwitchChannel {
        CmdSwitchChannel
    }

    fn create() -> BoxedCmdDescriptor {
        box CmdSwitchChannel::new() as BoxedCmdDescriptor
    }
}

impl CmdDescriptor for CmdSwitchChannel {
    fn name(&self) -> &'static str {
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
    command_desciptors: Vec<BoxedCmdDescriptor>,
    current_channel: Option<String>,
}

impl<'a> UserInterface<'a> {
    fn new<'a>(conn: &'a mut IrcConnection) -> UserInterface<'a> {
        let mut commands = Vec::new();
        commands.push(box CmdNames::create());
        commands.push(box CmdJoin::create());
        commands.push(box CmdSwitchChannel::create());
        UserInterface {
            connection: conn,
            current_phase: Registration,
            command_desciptors: Vec::new(),
            current_channel: None
        }
    }

    fn parse_command<'a>(line: &'a str) -> Option<(&'a str, &'a str)> {
        if line.starts_with(COMMAND_PREFIX) {
            Some(match line.find(' ') {
                Some(idx) => (
                    line[COMMAND_PREFIX.len()..idx],
                    line[idx+1..]
                ),
                None => (
                    line[COMMAND_PREFIX.len()..],
                    ""
                )
            })
        } else {
            None
        }
    }

    fn get_current_prompt(&mut self) -> SendStr {
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

    fn run_interface_registration(&mut self, tx: SyncSender<UiCommand>) {
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

    fn find_command(commands: &'a Vec<BoxedCmdDescriptor>, command_name: &str)
                    -> Option<&'a BoxedCmdDescriptor> {
        for command_desc in commands.iter() {
            if command_name == command_desc.name() {
                return Some(command_desc)
            }
        }
        None
    }

    fn run_interface_connected(&mut self, tx: SyncSender<UiCommand>) {
        let prompt = self.get_current_prompt();
        let line = match readline::readline(prompt.as_slice()) {
            Some(line) => line,
            None => return
        };
        let line_cleaned = line[].trim_chars('\n');

        let descs = &self.command_desciptors;
        let command_pair = match UserInterface::parse_command(line_cleaned) {
            Some((command, rest)) => {
                match UserInterface::find_command(descs, command) {
                    Some(command_iface) => Some((command_iface, rest)),
                    None => {
                        PrintLn(format!("unknown command: {}", command));
                        println!("unknown command: {}", command);
                        None
                    }
                }
            },
            None => {
                self.connection.write_str(line_cleaned);
                None
            }
        };
        match command_pair {
            Some((iface, rest)) => {
                ;;
            },
            None => ()
        }
    }

    fn run_interface_disconnected(&mut self, tx: SyncSender<UiCommand>) {
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

    fn run_interface(&mut self, events: Receiver<IrcEvent>) {
        let (tx, rx) = sync_channel::<UiCommand>(1);
        spawn(proc() {
            let mut stdout_w = std::io::stdout();
            let write_prompt = |line: SendStr| {
                try!(stdout_w.write_str(format!("{}", line.as_slice())[]));
                try!(stdout_w.flush())
                Ok(())
            };

            for ui_cmd in rx.iter() {
                match ui_cmd {
                    UpdatePrompt(new_prompt) => {
                        println!("\r{}", new_prompt.as_slice());
                        assert!(write_prompt(new_prompt).is_ok());
                    },
                    PrintLn(line) => {
                        println!("\r{}", line.as_slice())
                    }
                }
            }
        });

        loop {
            let tx_clone = tx.clone();
            match self.current_phase {
                Registration => self.run_interface_registration(tx_clone),
                Connected => self.run_interface_connected(tx_clone),
                Disconnected => self.run_interface_disconnected(tx_clone),
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

    // spawn(proc() {
    //     for event in eventstream.iter() {
    //         if let IrcEventMessage(message) = event {
    //             println!("RX: {}", message);
    //         }
    //     }
    // });

    let mut ui = UserInterface::new(&mut conn);
    ui.run_interface(eventstream);   
}
