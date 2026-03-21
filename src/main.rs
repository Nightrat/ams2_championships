mod championship_html;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path/to/Championships.xml>", args[0]);
        std::process::exit(1);
    }
    let xml_path = &args[1];
    let output_path = "championships.html";

    if let Err(e) = championship_html::convert(xml_path, output_path) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
