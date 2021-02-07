use noisy_float::prelude::*;
use std::io::BufRead;

use log::debug;

use crate::event::Event;

pub fn parse_event<B: BufRead>(reader: &mut B) -> Option<Event> {
    let mut event = Event::new();

    let mut buf = String::new();
    if reader.read_line(&mut buf).unwrap() == 0 {
        debug!("reached EOF");
        return None;
    }
    event.id = buf.trim().parse().unwrap();

    let mut buf = String::new();
    reader.read_line(&mut buf).unwrap();
    event.weight = n64(buf.trim().parse().unwrap());

    let mut buf = String::new();
    reader.read_line(&mut buf).unwrap();
    let ntypes: usize = buf.trim().parse().unwrap();

    let mut outgoing_by_pid = Vec::with_capacity(ntypes);
    for _ in 0..ntypes {
        let mut buf = Vec::new();
        reader.read_until(b' ', &mut buf).unwrap();

        let t = std::str::from_utf8(&buf).unwrap().trim().parse().unwrap();

        let mut buf = String::new();
        reader.read_line(&mut buf).unwrap();

        let nmomenta = buf.trim().parse().unwrap();

        let mut momenta = Vec::with_capacity(nmomenta);
        for _ in 0..nmomenta {
            let mut buf = String::new();
            reader.read_line(&mut buf).unwrap();

            let mut p = [n64(0.); 4];
            for (n, pi) in buf.split(' ').enumerate() {
                p[n] = n64(pi.trim().parse().unwrap());
            }
            momenta.push(p.into());
        }
        outgoing_by_pid.push((t, momenta));
    }
    event.outgoing_by_pid = outgoing_by_pid;
    Some(event)
}
