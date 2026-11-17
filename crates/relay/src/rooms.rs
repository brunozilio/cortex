use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::mpsc;
use tunnel::Frame;

use crate::Limits;

pub type ToMachine = mpsc::Sender<Frame>;
pub type ToPhone = mpsc::Sender<Vec<u8>>;

#[derive(Debug)]
struct Room {
    machine: ToMachine,
    phones: HashMap<u32, ToPhone>,
    next: u32,
    generation: u64,
}

#[derive(Debug, Default)]
struct Hall {
    rooms: HashMap<String, Room>,
    generation: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Rooms(Arc<Mutex<Hall>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Taken,
    Full,
    Absent,
}

#[derive(Debug)]
pub struct Claim {
    rooms: Rooms,
    id: String,
    generation: u64,
}

#[derive(Debug)]
pub struct Seat {
    rooms: Rooms,
    machine: String,
    pub channel: u32,
    pub to_machine: ToMachine,
}

pub fn valid_machine_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl Rooms {
    fn hall(&self) -> MutexGuard<'_, Hall> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn holds(&self, id: &str) -> bool {
        self.hall().rooms.contains_key(id)
    }

    pub fn claim(&self, id: &str, machine: ToMachine, limits: &Limits) -> Result<Claim, Refused> {
        let mut hall = self.hall();
        if hall.rooms.contains_key(id) {
            return Err(Refused::Taken);
        }
        if hall.rooms.len() >= limits.machines {
            return Err(Refused::Full);
        }
        hall.generation += 1;
        let generation = hall.generation;
        hall.rooms.insert(id.to_owned(), Room { machine, phones: HashMap::new(), next: 1, generation });
        Ok(Claim { rooms: self.clone(), id: id.to_owned(), generation })
    }

    pub fn seat(&self, machine: &str, phone: ToPhone, limits: &Limits) -> Result<Seat, Refused> {
        let mut hall = self.hall();
        let room = hall.rooms.get_mut(machine).ok_or(Refused::Absent)?;
        if room.phones.len() >= limits.phones_per_machine {
            return Err(Refused::Full);
        }
        let channel = room.next;
        room.next = room.next.wrapping_add(1).max(1);
        room.phones.insert(channel, phone);
        Ok(Seat { rooms: self.clone(), machine: machine.to_owned(), channel, to_machine: room.machine.clone() })
    }

    pub fn phone(&self, machine: &str, channel: u32) -> Option<ToPhone> {
        self.hall().rooms.get(machine).and_then(|room| room.phones.get(&channel)).cloned()
    }

    pub fn hang_up(&self, machine: &str, channel: u32) {
        if let Some(room) = self.hall().rooms.get_mut(machine) {
            room.phones.remove(&channel);
        }
    }

    pub fn machines(&self) -> usize {
        self.hall().rooms.len()
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        let mut hall = self.rooms.hall();
        if hall.rooms.get(&self.id).is_some_and(|room| room.generation == self.generation) {
            hall.rooms.remove(&self.id);
        }
    }
}

impl Drop for Seat {
    fn drop(&mut self) {
        let mut hall = self.rooms.hall();
        let Some(room) = hall.rooms.get_mut(&self.machine) else { return };
        if room.phones.remove(&self.channel).is_some() {
            let _ = room.machine.try_send(Frame::Close(self.channel));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a failed step fails the test")]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef0123456789abcdef";

    fn limits() -> Limits {
        Limits { machines: 2, phones_per_machine: 2, ..Limits::default() }
    }

    #[test]
    fn a_machine_id_is_32_lowercase_hex_digits() {
        assert!(valid_machine_id(ID));
        assert!(!valid_machine_id(&ID.to_uppercase()));
        assert!(!valid_machine_id(&ID[1..]));
        assert!(!valid_machine_id("../../etc/passwd/0123456789abcdef"));
    }

    #[test]
    fn a_machine_id_has_one_owner_until_it_lets_go() {
        let rooms = Rooms::default();
        let (machine, _) = mpsc::channel(1);
        let claim = rooms.claim(ID, machine.clone(), &limits()).unwrap();
        assert_eq!(rooms.claim(ID, machine.clone(), &limits()).unwrap_err(), Refused::Taken);
        drop(claim);
        assert!(rooms.claim(ID, machine, &limits()).is_ok());
    }

    #[test]
    fn the_relay_takes_no_more_machines_than_its_limit() {
        let rooms = Rooms::default();
        let (machine, _) = mpsc::channel(1);
        let _first = rooms.claim(&"a".repeat(32), machine.clone(), &limits()).unwrap();
        let _second = rooms.claim(&"b".repeat(32), machine.clone(), &limits()).unwrap();
        assert_eq!(rooms.claim(&"c".repeat(32), machine, &limits()).unwrap_err(), Refused::Full);
    }

    #[test]
    fn phones_get_their_own_channels_up_to_the_limit_and_the_machine_hears_when_one_leaves() {
        let rooms = Rooms::default();
        let (machine, mut heard) = mpsc::channel(4);
        let _claim = rooms.claim(ID, machine, &limits()).unwrap();
        let (phone, _) = mpsc::channel(1);
        let first = rooms.seat(ID, phone.clone(), &limits()).unwrap();
        let second = rooms.seat(ID, phone.clone(), &limits()).unwrap();
        assert_ne!(first.channel, second.channel);
        assert_eq!(rooms.seat(ID, phone.clone(), &limits()).unwrap_err(), Refused::Full);
        let gone = first.channel;
        drop(first);
        assert_eq!(heard.try_recv().unwrap(), Frame::Close(gone));
        assert!(rooms.phone(ID, gone).is_none());
        assert!(rooms.seat(ID, phone, &limits()).is_ok());
    }

    #[test]
    fn a_phone_cannot_sit_where_no_machine_is() {
        let (phone, _) = mpsc::channel(1);
        assert_eq!(Rooms::default().seat(ID, phone, &limits()).unwrap_err(), Refused::Absent);
    }

    #[test]
    fn a_phone_the_machine_hung_up_on_does_not_echo_a_close_back() {
        let rooms = Rooms::default();
        let (machine, mut heard) = mpsc::channel(4);
        let _claim = rooms.claim(ID, machine, &limits()).unwrap();
        let (phone, _) = mpsc::channel(1);
        let seat = rooms.seat(ID, phone, &limits()).unwrap();
        rooms.hang_up(ID, seat.channel);
        drop(seat);
        assert!(heard.try_recv().is_err());
    }
}
