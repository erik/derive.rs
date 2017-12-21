#[macro_use]
extern crate serde_derive;
extern crate docopt;
extern crate image;
extern crate gpx;
extern crate geo;
extern crate time;

use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path;

use docopt::Docopt;
use gpx::read;
use gpx::{Gpx, Track};
use geo::Point;
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
    arg_directory: String,
}

#[derive(Debug)]
struct Activity {
    name: String,
    date: time::Tm,
    track_points: Vec<Point<f64>>,
}

fn parse_gpx(path: &path::Path) -> Option<Activity> {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);

    let gpx: Gpx = read(reader).unwrap();

    // Nothing to do if there are no tracks
    if gpx.tracks.len() == 0 {
        return None;
    } else if gpx.tracks.len() > 1 {
        println!("Warning! more than 1 track, just taking first");
    }

    let track: &Track = &gpx.tracks[0];

    let mut activity = Activity {
        name: String::from("unnamed"),
        date: time::empty_tm(),
        track_points: vec![],
    };

    if let Some(ref name) = track.name {
        activity.name = name.clone();
    }

    if let Some(metadata) = gpx.metadata {
        if let Some(_time) = metadata.time {
            // FIXME: update this
            activity.date = time::now();
        }
    }

    for seg in track.segments.iter() {
        let points = seg.points.iter().map(|ref wpt| wpt.point());
        activity.track_points.extend(points);
    }

    Some(activity)
}


fn main() {
    let args: CommandArgs = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    println!("{:?}", args);

    let dir_entry = fs::read_dir(args.arg_directory).unwrap();
    let files: Vec<path::PathBuf> = dir_entry.map(|p| p.unwrap().path()).collect();

    for path in files {
        let activity = parse_gpx(path.as_path());
        println!("Activity: {:?}", activity.unwrap().name);
    }
}
