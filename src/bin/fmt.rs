use jomini::{ArrayReader, Encoding, ObjectReader, TextTape, TextToken, ValueReader};
use std::{
    env,
    io::{stdout, Write},
};

fn main() {
    let args: Vec<String> = env::args().collect();
    let file_data = std::fs::read(&args[1]).unwrap();
    let tape = TextTape::from_slice(&file_data).unwrap();
    let reader = tape.utf8_reader();
    let stdout = stdout();
    let mut stdout_lock = stdout.lock();
    format_object_core(reader, 0, &mut stdout_lock)
}

fn format_value<E, W>(reader: ValueReader<E>, depth: u16, writer: &mut W)
where
    E: Encoding + Clone,
    W: Write,
{
    match reader.token() {
        TextToken::Unquoted(_) => {
            let _ = write!(writer, "{}", reader.read_str().unwrap());
        }
        TextToken::Quoted(_) => {
            let _ = write!(writer, "\"{}\"", reader.read_str().unwrap());
        }
        TextToken::Array(_) => format_array(reader.read_array().unwrap(), depth + 1, writer),
        TextToken::Object(_) => {
            format_object(reader.read_object().unwrap(), depth + 1, writer);
        }

        TextToken::HiddenObject(_) => {
            format_object_core(reader.read_object().unwrap(), 0, writer);
        }

        TextToken::Header(_) => {
            let mut header_reader = reader.read_array().unwrap();
            let scalar = header_reader.next_value().unwrap().read_str().unwrap();
            let _ = write!(writer, "{}", scalar);
            format_array(
                header_reader.next_value().unwrap().read_array().unwrap(),
                depth + 1,
                writer,
            )
        }

        // parameters should not be seen as values
        TextToken::End(_)
        | TextToken::Operator(_)
        | TextToken::Parameter(_)
        | TextToken::UndefinedParameter(_) => {
            panic!("unexpected syntax {:?}", reader.token());
        }
    }
}

fn format_object_core<E, W>(mut reader: ObjectReader<E>, depth: u16, writer: &mut W)
where
    E: Encoding + Clone,
    W: Write,
{
    while let Some((key, _op, value)) = reader.next_field() {
        for _ in 0..depth {
            let _ = writer.write(b" ");
        }
        let _ = write!(writer, "{}", key.read_str());
        let _ = writer.write(b"=");
        format_value(value, depth, writer);
        let _ = writer.write(b"\r\n");
    }
}

fn format_object<E, W>(reader: ObjectReader<E>, depth: u16, writer: &mut W)
where
    E: Encoding + Clone,
    W: Write,
{
    let _ = writer.write(b"{\r\n");

    format_object_core(reader, depth, writer);

    for _ in 0..depth - 1 {
        let _ = writer.write(b" ");
    }

    let _ = writer.write(b"}");
}

fn format_array<E, W>(mut reader: ArrayReader<E>, depth: u16, writer: &mut W)
where
    E: Encoding + Clone,
    W: Write,
{
    let _ = writer.write(b"{\r\n");
    while let Some(value) = reader.next_value() {
        for _ in 0..depth {
            let _ = writer.write(b" ");
        }
        format_value(value, depth, writer);
        let _ = writer.write(b"\r\n");
    }

    for _ in 0..depth - 1 {
        let _ = writer.write(b" ");
    }

    let _ = writer.write(b"}");
}
