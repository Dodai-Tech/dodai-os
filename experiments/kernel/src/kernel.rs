//! Tasks, messages, capabilities and the cooperative scheduler.

use crate::queue::Queue;
use hibari_device::{Capability, CapabilitySet};

/// Identifies a task.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TaskId(pub u8);

/// A fixed-size message between tasks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Message {
    /// Sender.
    pub from: TaskId,
    /// Receiver.
    pub to: TaskId,
    /// What kind of message it is; meaning is agreed between the two tasks.
    pub tag: u16,
    /// A small payload (a value, or a handle to a shared buffer).
    pub data: u32,
}

/// Why a kernel call failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// All task slots are in use.
    NoSlot,
    /// The task does not exist (never spawned, or already killed).
    NoSuchTask,
    /// The receiver's inbox is full.
    QueueFull,
    /// The task does not hold the capability.
    Denied(Capability),
}

#[derive(Clone, Copy, Debug)]
struct Slot<const Q: usize> {
    caps: CapabilitySet,
    inbox: Queue<Message, Q>,
}

/// The kernel: up to `T` tasks, each with an inbox of `Q` messages. All memory is inline, so the
/// size is known at build time and nothing allocates.
///
/// This first version is cooperative: tasks are event-driven state machines that run to
/// completion when [`next_ready`](Self::next_ready) hands them a message. No context switch is
/// needed, so it contains no assembly and no `unsafe`.
#[derive(Clone, Copy, Debug)]
pub struct Kernel<const T: usize, const Q: usize> {
    slots: [Option<Slot<Q>>; T],
    cursor: usize,
}

impl<const T: usize, const Q: usize> Default for Kernel<T, Q> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const T: usize, const Q: usize> Kernel<T, Q> {
    /// A kernel with no tasks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: [None; T],
            cursor: 0,
        }
    }

    fn slot(&self, id: TaskId) -> Result<&Slot<Q>, Error> {
        self.slots
            .get(usize::from(id.0))
            .and_then(Option::as_ref)
            .ok_or(Error::NoSuchTask)
    }

    fn slot_mut(&mut self, id: TaskId) -> Result<&mut Slot<Q>, Error> {
        self.slots
            .get_mut(usize::from(id.0))
            .and_then(Option::as_mut)
            .ok_or(Error::NoSuchTask)
    }

    /// Starts a task holding `caps`.
    ///
    /// # Errors
    /// [`Error::NoSlot`] when every slot is taken.
    pub fn spawn(&mut self, caps: CapabilitySet) -> Result<TaskId, Error> {
        let (i, slot) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, s)| s.is_none())
            .ok_or(Error::NoSlot)?;
        let id = u8::try_from(i).map_err(|_| Error::NoSlot)?;
        *slot = Some(Slot {
            caps,
            inbox: Queue::new(),
        });
        Ok(TaskId(id))
    }

    /// Stops a task and drops its pending messages.
    ///
    /// # Errors
    /// [`Error::NoSuchTask`] when it does not exist.
    pub fn kill(&mut self, id: TaskId) -> Result<(), Error> {
        self.slot(id)?;
        self.slots[usize::from(id.0)] = None;
        Ok(())
    }

    /// Queues a message from `from` to `to`.
    ///
    /// # Errors
    /// [`Error::NoSuchTask`] when either task is missing, [`Error::QueueFull`] when the inbox is full.
    pub fn send(&mut self, from: TaskId, to: TaskId, tag: u16, data: u32) -> Result<(), Error> {
        self.slot(from)?;
        self.slot_mut(to)?
            .inbox
            .push(Message {
                from,
                to,
                tag,
                data,
            })
            .map_err(|_| Error::QueueFull)
    }

    /// Takes the oldest message waiting for `id`.
    pub fn recv(&mut self, id: TaskId) -> Option<Message> {
        self.slot_mut(id).ok()?.inbox.pop()
    }

    /// The capabilities a task holds.
    #[must_use]
    pub fn caps(&self, id: TaskId) -> Option<CapabilitySet> {
        self.slot(id).ok().map(|s| s.caps)
    }

    /// Fails unless the task holds `cap`.
    ///
    /// # Errors
    /// [`Error::Denied`] when it does not, [`Error::NoSuchTask`] when the task is missing.
    pub fn require(&self, id: TaskId, cap: Capability) -> Result<(), Error> {
        if self.slot(id)?.caps.contains(cap) {
            Ok(())
        } else {
            Err(Error::Denied(cap))
        }
    }

    /// Lets `from` hand a capability it holds to `to`. Capabilities can be passed on, never created.
    ///
    /// # Errors
    /// [`Error::Denied`] when `from` does not hold `cap`, [`Error::NoSuchTask`] when a task is missing.
    pub fn delegate(&mut self, from: TaskId, to: TaskId, cap: Capability) -> Result<(), Error> {
        self.require(from, cap)?;
        let slot = self.slot_mut(to)?;
        slot.caps = slot.caps.with(cap);
        Ok(())
    }

    /// The next task with a waiting message, fairly (round robin).
    pub fn next_ready(&mut self) -> Option<TaskId> {
        for k in 0..T {
            let i = (self.cursor + k) % T;
            if self.slots[i].as_ref().is_some_and(|s| !s.inbox.is_empty()) {
                self.cursor = (i + 1) % T;
                return u8::try_from(i).ok().map(TaskId);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Capability::{Display, Touch, Wifi};

    type K = Kernel<4, 2>;

    #[test]
    fn spawn_fills_slots_then_fails() {
        let mut k = K::new();
        for i in 0..4 {
            assert_eq!(k.spawn(CapabilitySet::EMPTY), Ok(TaskId(i)));
        }
        assert_eq!(k.spawn(CapabilitySet::EMPTY), Err(Error::NoSlot));
        k.kill(TaskId(1)).unwrap();
        assert_eq!(k.spawn(CapabilitySet::EMPTY), Ok(TaskId(1)));
    }

    #[test]
    fn messages_flow_in_order_and_respect_the_inbox_size() {
        let mut k = K::new();
        let a = k.spawn(CapabilitySet::EMPTY).unwrap();
        let b = k.spawn(CapabilitySet::EMPTY).unwrap();
        assert_eq!(k.send(a, b, 1, 10), Ok(()));
        assert_eq!(k.send(a, b, 2, 20), Ok(()));
        assert_eq!(k.send(a, b, 3, 30), Err(Error::QueueFull));
        assert_eq!(
            k.recv(b),
            Some(Message {
                from: a,
                to: b,
                tag: 1,
                data: 10
            })
        );
        assert_eq!(k.recv(b).map(|m| m.tag), Some(2));
        assert_eq!(k.recv(b), None);
    }

    #[test]
    fn missing_tasks_are_reported() {
        let mut k = K::new();
        let a = k.spawn(CapabilitySet::EMPTY).unwrap();
        assert_eq!(k.send(a, TaskId(3), 0, 0), Err(Error::NoSuchTask));
        assert_eq!(k.send(TaskId(3), a, 0, 0), Err(Error::NoSuchTask));
        assert_eq!(k.send(a, TaskId(200), 0, 0), Err(Error::NoSuchTask));
        assert_eq!(k.kill(TaskId(3)), Err(Error::NoSuchTask));
    }

    #[test]
    fn capabilities_are_checked_and_only_passed_on() {
        let mut k = K::new();
        let ui = k.spawn(CapabilitySet::of(&[Display, Touch])).unwrap();
        let app = k.spawn(CapabilitySet::EMPTY).unwrap();
        assert_eq!(k.require(app, Display), Err(Error::Denied(Display)));
        assert_eq!(k.delegate(ui, app, Display), Ok(()));
        assert_eq!(k.require(app, Display), Ok(()));
        assert_eq!(k.delegate(ui, app, Wifi), Err(Error::Denied(Wifi)));
        assert_eq!(k.delegate(app, ui, Touch), Err(Error::Denied(Touch)));
    }

    #[test]
    fn scheduler_is_round_robin_over_ready_tasks() {
        let mut k = K::new();
        let a = k.spawn(CapabilitySet::EMPTY).unwrap();
        let b = k.spawn(CapabilitySet::EMPTY).unwrap();
        assert_eq!(k.next_ready(), None);
        k.send(a, a, 0, 0).unwrap();
        k.send(a, a, 0, 1).unwrap();
        k.send(a, b, 0, 2).unwrap();
        assert_eq!(k.next_ready(), Some(a));
        k.recv(a);
        assert_eq!(k.next_ready(), Some(b));
        k.recv(b);
        assert_eq!(k.next_ready(), Some(a));
        k.recv(a);
        assert_eq!(k.next_ready(), None);
    }

    #[test]
    fn killed_task_loses_its_messages() {
        let mut k = K::new();
        let a = k.spawn(CapabilitySet::EMPTY).unwrap();
        let b = k.spawn(CapabilitySet::EMPTY).unwrap();
        k.send(a, b, 0, 0).unwrap();
        k.kill(b).unwrap();
        assert_eq!(k.recv(b), None);
        assert_eq!(k.next_ready(), None);
    }

    #[test]
    fn zero_tasks_is_fine() {
        let mut k = Kernel::<0, 0>::new();
        assert_eq!(k.next_ready(), None);
        assert_eq!(k.spawn(CapabilitySet::EMPTY), Err(Error::NoSlot));
    }
}
