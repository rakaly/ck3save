use jomini::{TextTape, TextWriterBuilder};
use std::{
    env,
    io::{stdout, BufWriter, Write},
};

fn main() {
    let args: Vec<String> = env::args().collect();
    let file_data = std::fs::read(&args[1]).unwrap();
    let tape = TextTape::from_slice(&file_data).unwrap();
    let stdout = stdout();
    let stdout_lock = stdout.lock();
    let buf_stdout = BufWriter::new(stdout_lock);
    let mut writer = TextWriterBuilder::new().from_writer(buf_stdout);
    writer.write_tape(&tape).unwrap();
    let mut buf_stdout = writer.into_inner();
    buf_stdout.flush().unwrap();
}
