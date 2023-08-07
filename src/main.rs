use std::error::Error;
use std::io::{stdin, stdout, Write};
use std::{fs::File, sync::Arc, sync::Mutex};

use itertools::interleave;
use midir::{Ignore, MidiInput};
use rustysynth::{SoundFont, SynthesizerSettings, Synthesizer};
use tinyaudio::{OutputDeviceParameters, run_output_device};

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut input = String::new();

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    // Get an input port (read from console if multiple are available)
    let in_ports = midi_in.ports();
    let in_port = match in_ports.len() {
        0 => return Err("no input port found".into()),
        1 => {
            println!(
                "Choosing the only available input port: {}",
                midi_in.port_name(&in_ports[0]).unwrap()
            );
            &in_ports[0]
        }
        _ => {
            println!("\nAvailable input ports:");
            for (i, p) in in_ports.iter().enumerate() {
                println!("{}: {}", i, midi_in.port_name(p).unwrap());
            }
            print!("Please select input port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            in_ports
                .get(input.trim().parse::<usize>()?)
                .ok_or("invalid input port selected")?
        }
    };

    println!("\nOpening connection");
    let in_port_name = midi_in.port_name(in_port)?;

    // The output buffer (3 seconds).
    let sample_rate = 44100;
    let sample_count = 64 as usize;
    let mut left: Vec<f32> = vec![0_f32; sample_count];
    let mut right: Vec<f32> = vec![0_f32; sample_count];

    let params: OutputDeviceParameters = OutputDeviceParameters {
        channels_count: 2,
        sample_rate: sample_rate,
        channel_sample_count: sample_count,
    };

    // Load the SoundFont.
    // let mut sf2 = File::open("FullConcertGrandV3.sf2").unwrap();
    // let mut sf2 = File::open("Essential Keys-sforzando-v9.6.sf2").unwrap();
    let mut sf2 = File::open("Steinway-Chateau-Plus-Instruments-v1.7.sf2").unwrap();
    let sound_font: Arc<SoundFont> = Arc::new(SoundFont::new(&mut sf2).unwrap());

    // Create the synthesizer.
    let settings: SynthesizerSettings = SynthesizerSettings::new(params.sample_rate.try_into().unwrap());
    let synthesizer = Arc::new(Mutex::new(Synthesizer::new(&sound_font, &settings).unwrap()));

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    let synthesizer_in = Arc::clone(&synthesizer);
    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |stamp, message, _| {
            println!("{}: {:?} (len = {})", stamp, message, message.len());
            match message.len() {
                1 => synthesizer_in.lock().unwrap().process_midi_message(0, message[0].into(), 0, 0),
                2 => synthesizer_in.lock().unwrap().process_midi_message(0, message[0].into(), message[1].into(), 0),
                3 => synthesizer_in.lock().unwrap().process_midi_message(0, message[0].into(), message[1].into(), message[2].into()),
                _ => panic!("Unknown MIDI message length")
            }
        },
        (),
    )?;

    println!(
        "Connection open, reading input from '{}' (press enter to exit) ...",
        in_port_name
    );

    // Render the waveform.
    let synthesizer_out = Arc::clone(&synthesizer);
    let _device = run_output_device(params, {
        move |data| {
            synthesizer_out.lock().unwrap().render(&mut left[..], &mut right[..]);
            for (i, value) in interleave(left.iter(), right.iter()).enumerate() {
                data[i] = *value * 2.0;
            }
        }
    })
    .unwrap();

    input.clear();
    stdin().read_line(&mut input)?; // wait for next enter key press

    println!("Closing connection");
    Ok(())
}

