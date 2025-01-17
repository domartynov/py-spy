// This code is adapted from rbspy:
// https://github.com/rbspy/rbspy/tree/master/src/ui/speedscope.rs
// licensed under the MIT License:
/*
MIT License

Copyright (c) 2016 Julia Evans, Kamal Marhubi
Portions (continuous integration setup) Copyright (c) 2016 Jorge Aparicio

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/

use std::collections::{HashMap};
use std::io;
use std::io::Write;
use std::fs::File;

use crate::stack_trace;
use remoteprocess::Tid;

use failure::{Error};
use serde_json;

/*
 * This file contains code to export rbspy profiles for use in https://speedscope.app
 *
 * The TypeScript definitions that define this file format can be found here:
 * https://github.com/jlfwong/speedscope/blob/9d13d9/src/lib/file-format-spec.ts
 *
 * From the TypeScript definition, a JSON schema is generated. The latest
 * schema can be found here: https://speedscope.app/file-format-schema.json
 *
 * This JSON schema conveniently allows to generate type bindings for generating JSON.
 * You can use https://app.quicktype.io/ to generate serde_json Rust bindings for the
 * given JSON schema.
 *
 * There are multiple variants of the file format. The variant we're going to generate
 * is the "type: sampled" profile, since it most closely maps to rbspy's data recording
 * structure.
 */

#[derive(Debug, Serialize)]
struct SpeedscopeFile {
    #[serde(rename = "$schema")]
    schema: String,
    profiles: Vec<Profile>,
    shared: Shared,

    #[serde(rename = "activeProfileIndex")]
    active_profile_index: Option<f64>,

    exporter: Option<String>,

    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct Profile {
    #[serde(rename = "type")]
    profile_type: ProfileType,

    name: String,
    unit: ValueUnit,

    #[serde(rename = "startValue")]
    start_value: f64,

    #[serde(rename = "endValue")]
    end_value: f64,

    samples: Vec<Vec<usize>>,
    weights: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Shared {
    frames: Vec<Frame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Frame {
    name: String,
    file: Option<String>,
    line: Option<u32>,
    col: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
enum ProfileType {
    #[serde(rename = "evented")]
    Evented,
    #[serde(rename = "sampled")]
    Sampled,
}

#[derive(Debug, Serialize, Deserialize)]
enum ValueUnit {
    #[serde(rename = "bytes")]
    Bytes,
    #[serde(rename = "microseconds")]
    Microseconds,
    #[serde(rename = "milliseconds")]
    Milliseconds,
    #[serde(rename = "nanoseconds")]
    Nanoseconds,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "seconds")]
    Seconds,
}

impl SpeedscopeFile {
  pub fn new(samples: &HashMap<Tid, Vec<Vec<usize>>>, frames: &Vec<Frame>) -> SpeedscopeFile {
    let end_value = samples.len();

    SpeedscopeFile {
      // This is always the same
      schema: "https://www.speedscope.app/file-format-schema.json".to_string(),

      active_profile_index: None,

      name: Some("py-spy profile".to_string()),

      exporter: Some(format!("py-spy@{}", env!("CARGO_PKG_VERSION"))),

      profiles: samples.iter().map(|(_, samples)| {
        let weights: Vec<f64> = (&samples).iter().map(|_s| 1_f64).collect();

        Profile {
            profile_type: ProfileType::Sampled,
            name: String::from("py-spy"),
            unit: ValueUnit::None,
            start_value: 0.0,
            end_value: end_value as f64,
            samples: samples.clone(),
            weights
        }
      }).collect(),

      shared: Shared {
          frames: frames.clone()
      }
    }
  }
}

impl Frame {
    pub fn new(stack_frame: &stack_trace::Frame) -> Frame {
        Frame {
            name: stack_frame.name.clone(),
            // TODO: filename?
            file: Some(stack_frame.filename.clone()),
            line: Some(stack_frame.line as u32),
            col: None
        }
    }
}

pub struct Stats {
    samples: HashMap<Tid, Vec<Vec<usize>>>,
    frames: Vec<Frame>,
    frame_to_index: HashMap<stack_trace::Frame, usize>
}

impl Stats {
    pub fn new() -> Stats {
        Stats {
            samples: HashMap::new(),
            frames: vec![],
            frame_to_index: HashMap::new()
        }
    }

    pub fn record(&mut self, stack: &stack_trace::StackTrace) -> Result<(), io::Error> {
        let mut frame_indices: Vec<usize> = stack.frames.iter().map(|frame| {
            let frames = &mut self.frames;
            *self.frame_to_index.entry(frame.clone()).or_insert_with(|| {
                let len = frames.len();
                frames.push(Frame::new(&frame));
                len
            })
        }).collect();
        frame_indices.reverse();

        self.samples.entry(stack.thread_id as Tid).or_insert_with(|| {
            vec![]
        }).push(frame_indices);
        Ok(())
    }

    pub fn write(&self, w: &mut File) -> Result<(), Error> {
        let json = serde_json::to_string(&SpeedscopeFile::new(&self.samples, &self.frames))?;
        writeln!(w, "{}", json)?;
        Ok(())
    }
}
