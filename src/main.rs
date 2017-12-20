#[macro_use]
extern crate serde_derive;
extern crate docopt;
extern crate image;
extern crate gpx;
extern crate quick_xml;

use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path;

use docopt::Docopt;
use gpx::read;
use gpx::Gpx;
use image::ImageBuffer;


const USAGE: &'static str = "
Generate video from GPX files.

Usage:
  derivers <top-lat> <left-lng> <bottom-lat> <right-lng> <width> <height> <directory>
";


#[derive(Debug, Deserialize)]
struct CommandArgs {
    arg_top_lat: f64,
    arg_left_lng: f64,
    arg_bottom_lat: f64,
    arg_right_lng: f64,
    arg_width: u32,
    arg_height: u32,
    arg_directory: String
}

fn parse_gpx(path: &path::Path) -> () {
}


fn main() {
    let args: CommandArgs = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    println!("{:?}", args);

    let paths = fs::read_dir(args.arg_directory).unwrap();
    for path in paths {
        let file_name = path.unwrap().path();
        let file = File::open(file_name.clone()).unwrap();
        let reader = BufReader::new(file);

        let gpx: Gpx = read(reader).unwrap();

        println!("Reading GPX file: {:?}", file_name.display());
    }
}
