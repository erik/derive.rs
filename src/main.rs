#[macro_use]
extern crate serde_derive;
extern crate docopt;
extern crate image;
extern crate gpx;
extern crate geo;
extern crate chrono;
extern crate rayon;

use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path;

use docopt::Docopt;
use gpx::read;
use gpx::{Gpx, Track};
use geo::Point;
use image::ImageBuffer;
use rayon::prelude::*;


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
struct ImageFrame {
    top_left: Point<f64>,
    bottom_right: Point<f64>,
    buf: image::RgbaImage
}

impl ImageFrame {
    pub fn from(args: &CommandArgs) -> ImageFrame {

        // h == w * (top - bottom) / (right - left)
        let height = (args.arg_width as f64) *
            ((args.arg_top_lat - args.arg_bottom_lat) /
             (args.arg_right_lng - args.arg_left_lng));

        println!("Computed height: {:?}", height);

        ImageFrame {
            top_left: Point::new(args.arg_left_lng, args.arg_top_lat),
            bottom_right: Point::new(args.arg_right_lng, args.arg_bottom_lat),
            buf: ImageBuffer::from_pixel(args.arg_width, height as u32,
                                         image::Rgba([0, 0, 0, 255]))
        }
    }

    // Using simple equirectangular projection for now
    pub fn project_to_screen(&self, coord: &Point<f64>) -> Option<(u32, u32)> {
        // lng is x pos
        let x_pos = self.top_left.lng() - coord.lng();
        let y_pos = self.top_left.lat() - coord.lat();

        let x_offset = x_pos / (self.top_left.lng() - self.bottom_right.lng());
        let y_offset = y_pos / (self.top_left.lat() - self.bottom_right.lat());

        let (x, y) = ((x_offset * self.buf.width() as f64),
                      (y_offset * self.buf.height() as f64));

        if (x < 0.0 || x as u32 >= self.buf.width()) ||
            (y < 0.0 || y as u32 >= self.buf.height()) {
            None
        } else {
            Some((x as u32, y as u32))
        }
    }
}


#[derive(Debug)]
struct Activity {
    name: String,
    date: chrono::DateTime<chrono::Utc>,
    track_points: Vec<Point<f64>>,
}

fn parse_gpx(path: &path::PathBuf) -> Option<Activity> {
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
        name: track.name.clone().unwrap_or(String::from("Untitled")),
        date: chrono::Utc::now(),
        track_points: vec![],
    };

    if let Some(metadata) = gpx.metadata {
        if let Some(time) = metadata.time {
            activity.date = time;
        }
    }

    // Append all the waypoints.
    for seg in track.segments.iter() {
        let points = seg.points.iter().map(|ref wpt| wpt.point());
        activity.track_points.extend(points);
    }

    if activity.track_points.len() == 0 {
        None
    } else {
        Some(activity)
    }
}


fn main() {
    let args: CommandArgs = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    println!("{:?}", args);

    let mut img = ImageFrame::from(&args);

    let paths: Vec<path::PathBuf> = fs::read_dir(args.arg_directory)
        .unwrap()
        .into_iter()
        .map(|p| p.unwrap().path())
        .collect();

    let activities: Vec<Activity> = paths
        .into_par_iter()
        .filter_map(|ref p| parse_gpx(p))
        .collect();

    for act in activities {
        println!("Activity: {:?}", act.name);

        for pt in act.track_points.iter() {
            if let Some((x, y)) = img.project_to_screen(pt) {
                let pixel = img.buf.get_pixel_mut(x, y);
                let c = if pixel[0] == 255 {
                    pixel[0]
                } else if pixel[0] == 0{
                    25
                } else {
                    pixel[0] + 5
                };

                *pixel = image::Rgba([c, c, c, 255]);
            }
        }
    }

    let fout = &mut File::create("heatmap.png").unwrap();
    image::ImageRgba8(img.buf).save(fout, image::PNG).unwrap();
}
