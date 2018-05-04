use super::{Room, RoomKind, Surface};
use dungeon::{Coord, Direction, Positioned, X, Y};
use error::{GameResult, ResultExt};
use fenwick::FenwickSet;
use fixedbitset::FixedBitSet;
use rect_iter::{IntoTuple2, RectRange};
use rng::{Rng, RngHandle};
use std::collections::HashMap;
use tuple_map::TupleMap2;

/// make passages between rooms
pub(crate) fn dig_passges<F>(
    rooms: &[Room],
    xrooms: X,
    yrooms: Y,
    rng: &mut RngHandle,
    max_extra_edges: usize,
    mut register: F,
) -> GameResult<()>
where
    F: FnMut(Positioned<Surface>) -> GameResult<()>,
{
    let mut graph = RoomGraph::new(xrooms, yrooms);
    let num_rooms = rooms.len();
    let mut selected = FenwickSet::with_capacity(num_rooms);
    let mut cur_room = rng.range(0..num_rooms);
    selected.insert(cur_room);
    // Connect all rooms
    while selected.len() < num_rooms {
        let nxt = select_candidate(num_rooms, &graph[cur_room], &selected, rng);
        if let Some((nxt_room, direction)) = nxt {
            selected.insert(nxt_room);
            graph.coonect(cur_room, nxt_room);
            connect_2rooms(
                &rooms[cur_room],
                &rooms[nxt_room],
                direction,
                rng,
                &mut register,
            ).chain_err("[passages::dig_passges]")?;
        } else {
            cur_room = selected.select(rng).unwrap();
        }
    }
    // Add some edges randomly so that there isn't always only one passage
    // between adjacent 2 rooms
    let try_num = rng.range(0..max_extra_edges);
    for _ in 0..try_num {
        let room1 = rng.range(0..num_rooms);
        let selected = select_candidate(num_rooms, &graph[room1], &selected, rng);
        if let Some((room2, direction)) = selected {
            graph.coonect(room1, room2);
            connect_2rooms(&rooms[room1], &rooms[room2], direction, rng, &mut register)
                .chain_err("[passages::dig_passages]")?;
        }
    }
    Ok(())
}

fn select_candidate(
    num_rooms: usize,
    node: &Node,
    selected: &FenwickSet,
    rng: &mut RngHandle,
) -> Option<(usize, Direction)> {
    (0..num_rooms)
        .filter_map(|i| {
            if selected.contains(i) {
                None
            } else {
                node.candidates.get(&i).map(|dir| (i, *dir))
            }
        })
        .enumerate()
        .filter(|(i, _)| rng.does_happen(*i as u32 + 1))
        .last()
        .map(|t| t.1)
}

fn connect_2rooms<F>(
    room1: &Room,
    room2: &Room,
    direction: Direction,
    rng: &mut RngHandle,
    register: &mut F,
) -> GameResult<()>
where
    F: FnMut(Positioned<Surface>) -> GameResult<()>,
{
    let (room1, room2, direction) = match direction {
        Direction::Up | Direction::Left => (room2, room1, direction.reverse()),
        _ => (room1, room2, direction),
    };
    let start = select_start_or_end(room1, direction, rng);
    let end = select_start_or_end(room2, direction.reverse(), rng);
    register(Positioned(start, door_kind(room1)))?;
    register(Positioned(end, door_kind(room2)))?;
    // decide where to turn randomly
    let (turn_start, turn_dir, turn_end) = match direction {
        Direction::Down => {
            if start.y.0 + 1 == end.y.0 {
                panic!("{:?} {:?}", room1.kind, room2.kind);
            }
            let y = rng.range(start.y.0 + 1..end.y.0);
            let dir = if start.is_lefter(end) {
                Direction::Right
            } else {
                Direction::Left
            };
            (Coord::new(start.x, y), dir, Coord::new(end.x, y))
        }
        Direction::Right => {
            let x = rng.range(start.x.0 + 1..end.x.0);
            let dir = if start.is_upper(end) {
                Direction::Down
            } else {
                Direction::Up
            };
            (Coord::new(x, start.y), dir, Coord::new(x, end.y))
        }
        _ => unreachable!(),
    };
    // dig passage from start to end
    start
        .direc_iter(direction, |cd| cd != turn_start)
        .skip(1)
        .chain(turn_start.direc_iter(turn_dir, |cd| cd != turn_end))
        .chain(turn_end.direc_iter(direction, |cd| cd != end))
        .try_for_each(|cd| register(Positioned(cd, Surface::Passage)))
        .chain_err("[passages::connect_2rooms]")
}

fn door_kind(room: &Room) -> Surface {
    match room.kind {
        RoomKind::Normal { .. } => Surface::Door,
        _ => Surface::Passage,
    }
}

fn select_start_or_end(room: &Room, direction: Direction, rng: &mut RngHandle) -> Coord {
    match room.kind {
        RoomKind::Normal { ref range } => {
            let candidates = inclusive_edges(range, direction);
            *rng.choose(&candidates).unwrap()
        }
        RoomKind::Maze {
            range: _,
            ref passages,
        } => *rng.choose(passages).unwrap(),
        RoomKind::Empty { up_left } => up_left,
    }
}

fn inclusive_edges(range: &RectRange<i32>, direction: Direction) -> Vec<Coord> {
    let bound_x = X(range.get_x().end - 1);
    let bound_y = Y(range.get_y().end - 1);
    match direction {
        Direction::Down => {
            let start: Coord = range.lower_left().into();
            start
                .slide_x(1)
                .direc_iter(Direction::Right, |cd: Coord| cd.x < bound_x)
                .collect()
        }
        Direction::Left => {
            let start: Coord = range.upper_left().into();
            start
                .slide_y(1)
                .direc_iter(Direction::Down, |cd| cd.y < bound_y)
                .collect()
        }
        Direction::Right => {
            let start: Coord = range.upper_right().into();
            start
                .slide_y(1)
                .direc_iter(Direction::Down, |cd| cd.y < bound_y)
                .collect()
        }
        Direction::Up => {
            let start: Coord = range.upper_left().into();
            start
                .slide_x(1)
                .direc_iter(Direction::Right, |cd| cd.x < bound_x)
                .collect()
        }
        _ => panic!(
            "[passages::connet_2rooms] invalid direction {:?}",
            direction
        ),
    }
}

/// room connectivity
#[derive(Clone, Debug, Index)]
struct RoomGraph {
    inner: Vec<Node>,
}

impl RoomGraph {
    fn new(xrooms: X, yrooms: Y) -> Self {
        let range = RectRange::zero_start(xrooms.0, yrooms.0).unwrap();
        let inner: Vec<_> = range
            .into_iter()
            .enumerate()
            .map(|(i, t)| Node::new(xrooms, yrooms, t, i))
            .collect();
        RoomGraph { inner }
    }
    fn coonect(&mut self, mut node1: usize, mut node2: usize) {
        self.inner[node1].connections.insert(node2);
        self.inner[node2].connections.insert(node1);
    }
}

/// node of room graph
#[derive(Clone, Debug)]
struct Node {
    connections: FixedBitSet,
    candidates: HashMap<usize, Direction>,
    id: usize,
}

impl Node {
    fn new(xrooms: X, yrooms: Y, room_pos: (i32, i32), id: usize) -> Self {
        let candidates: HashMap<_, _> = Direction::iter_variants()
            .take(4)
            .filter_map(|d| {
                let next = room_pos.add(d.to_cd().into_tuple2());
                if next.any(|a| a < 0) || next.0 >= xrooms.0 || next.1 >= yrooms.0 {
                    return None;
                }
                Some(((next.0 + next.1 * xrooms.0) as usize, d))
            })
            .collect();
        let num_rooms = (xrooms.0 * yrooms.0) as usize;
        Node {
            connections: FixedBitSet::with_capacity(num_rooms),
            candidates,
            id,
        }
    }
}

#[cfg_attr(test, test)]
fn test_inclusive_edges() {
    use std::ops::Range;
    let range = RectRange::from_ranges(5..10, 6..9).unwrap();
    let edge_vec = |xfix, fix, range: Range<i32>| -> Vec<_> {
        if xfix {
            range.map(|y| Coord::new(fix, y)).collect()
        } else {
            range.map(|x| Coord::new(x, fix)).collect()
        }
    };
    assert_eq!(
        inclusive_edges(&range, Direction::Down),
        edge_vec(false, 8, 6..9)
    );
    assert_eq!(
        inclusive_edges(&range, Direction::Up),
        edge_vec(false, 6, 6..9)
    );
    assert_eq!(
        inclusive_edges(&range, Direction::Left),
        edge_vec(true, 5, 7..8)
    );
    assert_eq!(
        inclusive_edges(&range, Direction::Right),
        edge_vec(true, 9, 7..8)
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use dungeon::rogue::rooms;
    use error::{ErrorId, ErrorKind};
    use rect_iter::{Get2D, GetMut2D};
    use std::collections::VecDeque;
    use Tile;
    fn to_buffer() -> Vec<Vec<Surface>> {
        let rooms = rooms::test::gen(10);
        let mut buffer = rooms::test::draw_to_buffer(&rooms);
        let mut rng = RngHandle::new();
        dig_passges(
            &rooms,
            X(3),
            Y(3),
            &mut rng,
            5,
            |Positioned(cd, surface)| {
                buffer
                    .try_get_mut_p(cd)
                    .and_then(|buf| {
                        *buf = surface;
                        Ok(())
                    })
                    .map_err(|e| {
                        let e: ErrorId = e.into();
                        e.into_err()
                    })
            },
        ).unwrap();
        buffer
    }
    #[test]
    #[ignore]
    fn print_passages() {
        let buffer = to_buffer();
        print_impl(&buffer);
    }
    fn print_impl(buffer: &Vec<Vec<Surface>>) {
        for v in buffer {
            for x in v {
                print!("{}", x.byte() as char);
            }
            println!("");
        }
    }
    #[test]
    fn connectivity() {
        for _ in 0..1000 {
            let buffer = to_buffer();
            let (xlen, ylen) = (buffer[0].len(), buffer.len());
            let start = RectRange::zero_start(xlen, ylen)
                .unwrap()
                .into_iter()
                .find(|&t| *buffer.get_p(t) == Surface::Floor)
                .map(|t| Coord::new(t.0 as i32, t.1 as i32))
                .unwrap();
            let mut visited = vec![vec![false; xlen]; ylen];
            *visited.get_mut_p(start) = true;
            let mut queue = VecDeque::new();
            queue.push_back(start);
            while let Some(cd) = queue.pop_front() {
                for dir in Direction::iter_variants().take(4) {
                    let nxt = cd + dir.to_cd();
                    if let Ok(s) = buffer.try_get_p(nxt) {
                        if s.can_walk() && !*visited.get_p(nxt) {
                            *visited.get_mut_p(nxt) = true;
                            queue.push_back(nxt);
                        }
                    }
                }
            }
            RectRange::zero_start(xlen, ylen)
                .unwrap()
                .into_iter()
                .for_each(|cd| {
                    if buffer.get_p(cd).can_walk() {
                        assert!(*visited.get_p(cd));
                    }
                });
        }
    }
}
