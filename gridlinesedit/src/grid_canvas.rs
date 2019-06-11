use imgui_sys_bindgen::sys::*;
use const_cstr::const_cstr;
use std::collections::{HashSet, HashMap};
use generational_arena::*;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Pt {
    x :i32,
    y :i32,
}

#[derive(Debug)]
pub struct Polyline {
    pts :Vec<Pt>,
}

pub fn length_maxmetric(p1 :Pt, p2 :Pt) -> i32 {
    ((p2.x - p1.x).abs()).max((p2.y - p1.y).abs())
}

impl Polyline {
    pub fn new() -> Self { Polyline { pts: Vec::new() } }
    pub fn is_empty(&self) -> bool { self.pts.is_empty() }
    pub fn reverse(&mut self) { self.pts.reverse(); }
    pub fn from_line(l :(Pt, Pt)) -> Polyline { Polyline { pts: vec![l.0, l.1] } }
    pub fn add_line(&mut self, l :(Pt, Pt)) -> Result<(),()> {
        self.add_polyline(Self::from_line(l)); 
        Ok(())
    }

    pub fn lengthmax(&self) -> i32 {
        self.pts.iter().zip(self.pts.iter().skip(1)).map(|(&p1,&p2)| length_maxmetric(p1,p2)).sum()
    }

    pub fn add_polyline(&mut self, mut pl :Polyline) -> Result<(),()> {
        if pl.is_empty() { return Ok(()); }
        if self.is_empty() { self.pts = pl.pts; return Ok(()); }

        let l1a = self.pts.first().unwrap();
        let l1b = self.pts.last().unwrap();
        let l2a = pl.pts.first().unwrap();
        let l2b = pl.pts.last().unwrap();

        let reverse_this = l1a == l2a || l1a == l2b;
        let reverse_other = l2a == l1a || l2a == l1b;

        if reverse_this { self.pts.reverse(); }
        if reverse_other { pl.pts.reverse(); }

        self.pts.pop();
        self.pts.extend(pl.pts);

        eprintln!("JOINED A POLYLINE {:?}", self.pts);

        if self.pts.first() == self.pts.last()  { return Err(()); }
        Ok(())
    }

    pub fn segments(&self) -> impl Iterator<Item = (&Pt,&Pt)> {
        self.points().zip(self.points().skip(1))
    }

    pub fn grid_step_internal<'a> (&'a self) -> impl Iterator<Item = Pt> + 'a {
        let last = self.pts.last().unwrap();
        self.segments().flat_map(|(&p1,&p2)| SchematicCanvas::step_line(p1,p2).skip(1))
            .filter(move |p| p != last)
    }

    pub fn grid_step<'a> (&'a self) -> impl Iterator<Item = Pt> + 'a {
        std::iter::once(*self.pts.first().unwrap()).chain(
            self.segments().flat_map(|(&p1,&p2)| SchematicCanvas::step_line(p1,p2).skip(1)))
    }

    pub fn split_at(&self, pt :Pt) -> Result<(Polyline, Polyline), ()> {
        if self.is_empty() { return Err(()); }

        let mut found = false;
        let (mut s1, mut s2) :(Vec<(Pt,Pt)>,Vec<(Pt,Pt)>) = (Vec::new(), Vec::new());
        for (&p1,&p2) in self.segments() {
            if !found  {
                if p1 == pt {
                    found = true;
                    s2.push((p1,p2));
                    continue;
                } else if let Some((start,end)) = split_line_at((p1,p2),pt) {
                    found = true;
                    s1.push(start); s2.push(end);
                    continue;
                }
            }

            if !found { s1.push((p1,p2)); } else { s2.push((p1,p2)); }
        }
        if found || self.pts.last().unwrap() == &pt { 
            Ok((Polyline::from_segments(s1)?,Polyline::from_segments(s2)?)) 
        } else { Err(()) }
    }

    pub fn from_segments(s :Vec<(Pt,Pt)>) -> Result<Polyline,()> {
        let mut pts = Vec::new();
        if s.is_empty() { return Ok(Polyline::new()); }

        pts.push(s.first().unwrap().0);

        for ((p1,p2),(p3,p4)) in s.iter().zip(s.iter().skip(1)) {
            if p2 != p3 { return Err(()); }
            pts.push(*p2);
        }

        pts.push(s.last().unwrap().1);
        Ok(Polyline { pts })
    }

    pub fn points(&self) -> impl Iterator<Item = &Pt> { self.pts.iter() }
}

pub fn split_line_at((p1,p2) :(Pt,Pt), pt :Pt) -> Option<((Pt,Pt),(Pt,Pt))> {
    if p1 == pt || p2 == pt { return None; }
    if pt.x < p1.x.min(p2.x) || pt.x > p1.x.max(p2.x) { return None; }
    if pt.y < p1.y.min(p2.y) || pt.y > p1.y.max(p2.y) { return None; }

    let dx1 = pt.x - p1.x;
    let dy1 = pt.y - p1.y;
    let dx2 = p2.x - pt.x;
    let dy2 = p2.y - pt.y;

    let same_x = dx1 == 0 || dx2 == 0 || dx1.signum() == dx2.signum();
    let same_y = dy1 == 0 || dy2 == 0 || dy1.signum() == dy2.signum();

    let same_xy1 = dx1 == 0 || dy1 == 0 || dx1.abs() == dy1.abs();
    let same_xy2 = dx2 == 0 || dy2 == 0 || dx2.abs() == dy2.abs();

    if same_x && same_y && same_xy1 && same_xy2 {
        Some(((p1,pt),(pt,p2)))
    } else {
        None
    }
}


// not a linear length/coordinates system, each 
// edge has a physical length which is not related
// to the dx on screen. 
#[derive(Debug)]
pub struct Railway {
    // NOTE that this structure requires keeping consistency between 
    // location.connections and track.end_a+end_b
    locations: Arena<Location>,
    tracks: Arena<Track>,
}

#[derive(Debug)]
pub struct SchematicCanvas {
    railway: Railway,
    selection: HashSet<Index>, // indexing locations. Select Track = select both a and b locations.

    // NOTE that this structure requires keeping consistency between railway, lines, and points.
    lines: HashMap<Index, Polyline>, 
    points: HashMap<Pt, PointInfo>,
    default_grid_resolution: f64,

    adding_line: Option<Pt>,
    scale: usize,
    translate :ImVec2,
}

#[derive(Debug)]
pub enum PointInfo {
    Location(Index),
    Track(Index),
}

#[derive(Debug)]
pub struct Track {
    end_a: Index,
    end_b: Index,
    length: f64,
}

#[derive(Debug)]
pub enum Dir { Up, Down }

#[derive(Debug)]
pub enum Side { Left, Right }

#[derive(Debug)]
pub struct Location {
    node: Option<Node>, // AUTO or Node specified
    connections: Vec<Index>, // Track ids
}

impl Location {
    pub fn empty() -> Location { Location {
        node: None,
        connections: Vec::new(),
    }}
}

#[derive(Debug)]
pub enum Node {
    End,
    Continue,
    Switch(Dir,Side),
    Crossing,
}

pub struct GridCanvas {
    lines: Vec<(Pt,Pt)>,
    adding_line: Option<Pt>,
    scale: usize,
    translate :ImVec2,
}

impl SchematicCanvas {
    pub fn new() -> Self {
        SchematicCanvas {
            railway: Railway {
                locations: Arena::new(),
                tracks: Arena::new(),
            },
            lines: HashMap::new(),
            points: HashMap::new(),
            adding_line: None,
            scale: 35, // number of pixels per grid point, in interval [4, 100]
            translate: ImVec2 { x: 0.0, y: 0.0 },
            default_grid_resolution: 50.0,
            selection :HashSet::new(),
        }
    }

    /// Converts and rounds a screen coordinate to the nearest point on the integer grid
    pub fn screen_to_world(&self, pt :ImVec2) -> Pt {
        let x = (self.translate.x + pt.x) / self.scale as f32;
        let y = (self.translate.y + pt.y) / self.scale as f32;
        Pt { x: x.round() as _ , y: y.round() as _ }
    }

    /// Convert a point on the integer grid into screen coordinates
    pub fn world_to_screen(&self, pt :Pt) -> ImVec2 {
        let x = ((self.scale as i32 * pt.x) as f32) - self.translate.x;
        let y = ((self.scale as i32 * pt.y) as f32) - self.translate.y;

        ImVec2 { x, y }
    }

    /// Return the rect of grid points within the current view.
    pub fn points_in_view(&self, size :ImVec2) -> (Pt,Pt) {
        let lo = self.screen_to_world(ImVec2 { x: 0.0, y: 0.0 });
        let hi = self.screen_to_world(size);
        (lo,hi)
    }

    pub fn route_line(from :Pt, to :Pt) -> Vec<(Pt,Pt)> {
        // diag
        let mut vec = Vec::new();
        let (dx,dy) = (to.x - from.x, to.y - from.y);
        let mut other = from;
        if dy.abs() > 0 {
            other = Pt { x: from.x + dy.abs() * dx.signum(), 
                         y: from.y + dy };
            vec.push((from, other));
        }
        if dx.abs() > 0 {
            let other_dx = to.x - other.x;
            let goal = Pt { x: other.x + if other_dx.signum() == dx.signum() { other_dx } else { 0 },
                            y: other.y };
            if other != goal {
                vec.push((other, goal));
            }
        }
        vec
    }

    pub fn step_line_internal(p1 :Pt, p2 :Pt) -> impl Iterator<Item = Pt> {
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        (1..(dx.abs().max(dy.abs()))).map(move |d| Pt { x: p1.x + d * dx.signum(), 
                                                        y: p1.y + d * dy.signum() } )
    }


    pub fn step_line(p1 :Pt, p2 :Pt) -> impl Iterator<Item = Pt> {
        eprintln!("STEP LINE {:?}  {:?}", p1,p2);
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;

        eprintln!(" p1 {:?}, p2 {:?}, dx {}, dy {}", p1, p2, dx, dy);

        // Ortholinear assumption
        assert!(dx.signum() != 0 || dy.signum() != 0);
        assert!(dx.signum() == 0 || dy.signum() == 0 || dx.abs() == dy.abs());

        eprintln!(" p1 {:?}, p2 {:?}, dx {}, dy {}", p1, p2, dx, dy);

        (0..=(dx.abs().max(dy.abs()))).map(move |d| Pt { x: p1.x + d * dx.signum(), 
                                                        y: p1.y + d * dy.signum() } )
    }

    pub fn add_line(&mut self, p1 :Pt, p2 :Pt) -> Result<(),()>  {
        let mut touched_nodes : HashSet<Index>  = HashSet::new();
        for (p1,p2) in Self::route_line(p1,p2) {

            // need to add new tracks and locs

            //
            // "cases":
            //  1. does not connect nor intersect anything
            //     --> add 2 locs and 1 track with both lines
            //  2. end connects to location only at one of the end points
            //     --> move the connected loc?
            //         and add the route_lines to the connected track
            //  3. ends connects to location at both end points
            //     --> merge tracks by adding lines
            //  4. ends connects to midpoint underay
            //     --> split the intersected track
            //  5. 
            //  6. overlap  ----> ignore
            //  7.
            //
            eprintln!("routed {:?}, {:?}", p1,p2);
            let pts = Self::step_line(p1,p2).collect::<Vec<_>>();
            for (p1,p2) in pts.iter().zip(pts.iter().skip(1)) {

                // now we have reduced the internal/external cases to 
                // just one type of new line, i.e. one of length 1.

                // if an end point is a track, split it

                eprintln!("adding {:?} {:?}", p1,p2);

                let loc1 = self.make_loc(*p1)?;
                let loc2 = self.make_loc(*p2)?;

                println!("loc1 {:?} loc2 {:?}", loc1, loc2);

                // create a new track joining the two locs.

                let t = self.railway.tracks.insert(Track { end_a: loc1, end_b: loc2, length: self.default_grid_resolution });
                self.railway.locations[loc1].connections.push(t);
                self.railway.locations[loc2].connections.push(t);

                self.points.insert(*p1, PointInfo::Location(loc1));
                self.points.insert(*p2, PointInfo::Location(loc2));
                self.lines.insert(t, Polyline::from_line((*p1,*p2)));

                touched_nodes.insert(loc1);
                touched_nodes.insert(loc2);
            }
        }

        for loc in touched_nodes {
            if self.railway.locations[loc].connections.len() == 2 {
                eprintln!("JOINING AT {:?}", loc);
                // can join
                let t1_id = self.railway.locations[loc].connections[0];
                let t2_id = self.railway.locations[loc].connections[1];
                let t1 = &self.railway.tracks[t1_id];
                let t2 = &self.railway.tracks[t2_id];

                if self.railway.tracks[t1_id].end_a == loc { self.reverse_track(t1_id); }
                if self.railway.tracks[t2_id].end_b == loc { self.reverse_track(t2_id); }

                for locref in self.railway.locations[ self.railway.tracks[t2_id].end_b ].connections.iter_mut() {
                    if *locref == t2_id {
                        *locref = t1_id;
                    }
                }

                self.railway.tracks[t1_id].length += self.railway.tracks[t2_id].length;
                self.railway.tracks[t1_id].end_b = self.railway.tracks[t2_id].end_b;

                let t2_pl = self.lines.remove(&t2_id).unwrap();
                for p in t2_pl.grid_step() {
                    self.points.insert(p, PointInfo::Track(t1_id));
                }
                self.lines.get_mut(&t1_id).unwrap().add_polyline(t2_pl);

                self.railway.tracks.remove(t2_id);
                self.railway.locations.remove(loc);
                

            }
        }

        Ok(())
    }

    pub fn reverse_track(&mut self, t :Index) {
        if let Some(mut t) = self.railway.tracks.get_mut(t) {
            std::mem::swap(&mut t.end_a, &mut t.end_b);
        }
        if let Some(mut t) = self.lines.get_mut(&t) {
            t.reverse();
        }
    }

    pub fn make_loc(&mut self, pt :Pt) -> Result<Index, ()> {
        match self.points.get(&pt) {
            Some(PointInfo::Location(l)) => Ok(*l), // ok
            Some(PointInfo::Track(t)) => {
                let t = *t;
                let pl = &self.lines[&t];
                let (pl1,pl2) = pl.split_at(pt)?;
                let (l1,l2,ls) = (pl1.lengthmax(), pl2.lengthmax(), pl.lengthmax());
                let loc = self.railway.locations.insert(Location::empty());
                self.lines.insert(t, pl1);
                let old_end = self.railway.tracks[t].end_b;
                let old_length = self.railway.tracks[t].length;
                self.railway.tracks[t].end_b = loc;
                self.railway.tracks[t].length = (l1 as f64 / ls as f64) * old_length;
                let new_t = self.railway.tracks.insert(Track { end_a: loc, end_b: old_end, 
                    length: (1.0 - (l1 as f64 / ls as f64))*old_length});

                for p in pl2.grid_step_internal() { self.points.insert(p,PointInfo::Track(new_t)); }
                self.points.insert(pt, PointInfo::Location(loc));

                self.lines.insert(new_t, pl2);
                Ok(loc)
                // omg too long...
            },
            None => {
                let loc = self.railway.locations.insert(Location::empty());
                self.points.insert(pt, PointInfo::Location(loc));
                Ok(loc)
            }
        }
    }
}

impl GridCanvas {
    pub fn new() -> Self {
        GridCanvas {
            lines: Vec::new(),
            adding_line: None,
            scale: 35, // number of pixels per grid point, in interval [4, 100]
            translate: ImVec2 { x: 0.0, y: 0.0 },
        }
    }

    /// Converts and rounds a screen coordinate to the nearest point on the integer grid
    pub fn screen_to_world(&self, pt :ImVec2) -> Pt {
        let x = (self.translate.x + pt.x) / self.scale as f32;
        let y = (self.translate.y + pt.y) / self.scale as f32;
        Pt { x: x.round() as _ , y: y.round() as _ }
    }

    /// Convert a point on the integer grid into screen coordinates
    pub fn world_to_screen(&self, pt :Pt) -> ImVec2 {
        let x = ((self.scale as i32 * pt.x) as f32) - self.translate.x;
        let y = ((self.scale as i32 * pt.y) as f32) - self.translate.y;

        ImVec2 { x, y }
    }

    /// Return the rect of grid points within the current view.
    pub fn points_in_view(&self, size :ImVec2) -> (Pt,Pt) {
        let lo = self.screen_to_world(ImVec2 { x: 0.0, y: 0.0 });
        let hi = self.screen_to_world(size);
        (lo,hi)
    }

    pub fn route_line(from :Pt, to :Pt) -> Vec<(Pt,Pt)> {
        // diag
        let mut vec = Vec::new();
        let (dx,dy) = (to.x - from.x, to.y - from.y);
        let mut other = from;
        if dy.abs() > 0 {
            other = Pt { x: from.x + dy.abs() * dx.signum(), 
                         y: from.y + dy };
            vec.push((from, other));
        }
        if dx.abs() > 0 {
            let other_dx = to.x - other.x;
            let goal = Pt { x: other.x + if other_dx.signum() == dx.signum() { other_dx } else { 0 },
                            y: other.y };
            if other != goal {
                println!(" route line ADDING {:?} {:?}", other,goal);
                vec.push((other, goal));
            }
        }
        vec
    }
}

pub fn schematic_canvas(size: &ImVec2, model: &mut SchematicCanvas) {
    unsafe {
        let io = igGetIO();
        let draw_list = igGetWindowDrawList();
        let pos = igGetCursorScreenPos_nonUDT2();
        let pos = ImVec2 { x: pos.x, y: pos.y };

        let c1 = igGetColorU32Vec4(ImVec4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 } );
        let c2 = igGetColorU32Vec4(ImVec4 { x: 0.2, y: 0.5, z: 0.95, w: 1.0 } );
        let c3 = igGetColorU32Vec4(ImVec4 { x: 1.0, y: 0.0, z: 1.0, w: 1.0 } );
        let c4 = igGetColorU32Vec4(ImVec4 { x: 0.8, y: 0.8, z: 0.8, w: 1.0 } );

        ImDrawList_AddRectFilled(draw_list,
                        pos, ImVec2 { x: pos.x + size.x, y: pos.y + size.y },
                        c1, 0.0, 0);
        igInvisibleButton(const_cstr!("grid_canvas").as_ptr(), *size);
        ImDrawList_PushClipRect(draw_list, pos, ImVec2 { x: pos.x + size.x, y: pos.y + size.y}, true);

        let pointer = (*io).MousePos;
        let pointer_incanvas = ImVec2 { x: pointer.x - pos.x, y: pointer.y - pos.y };
        let pointer_grid = model.screen_to_world(pointer_incanvas);

        let line = |c :ImU32,p1 :&ImVec2,p2 :&ImVec2| {
			ImDrawList_AddLine(draw_list,
				   ImVec2 { x: pos.x + p1.x, y: pos.y + p1.y },
				   ImVec2 { x: pos.x + p2.x, y: pos.y + p2.y },
				   c, 2.0);
        };

        // Drawing or adding line
        match (igIsItemHovered(0), igIsMouseDown(0), &mut model.adding_line) {
            (true, true, None)   => { model.adding_line = Some(pointer_grid); },
            (_, false, Some(pt)) => { 
                let pt = *pt;
                model.add_line(pt, pointer_grid); 
                model.adding_line = None;

                eprintln!(" NEW RAILWAY");
                eprintln!(" {:#?}", model);
            },
            _ => {},
        };

        // Draw permanent lines
        for (i,v) in &model.lines {
            for (p1,p2) in v.points().zip(v.points().skip(1)) {
                line(c2, &model.world_to_screen(*p1), &model.world_to_screen(*p2));
            }
        }

        // Draw temporary line
        if let Some(pt) = &model.adding_line {
            for (p1,p2) in SchematicCanvas::route_line(*pt, pointer_grid) {
                line(c3, &model.world_to_screen(p1), &model.world_to_screen(p2));
            }
        }

        // Draw grid + highlight on closest point if hovering?
        let (lo,hi) = model.points_in_view(*size);
        for x in lo.x..=hi.x {
            for y in lo.y..=hi.y {
                let pt = model.world_to_screen(Pt { x, y });
                ImDrawList_AddCircleFilled(draw_list, ImVec2 { x: pos.x + pt.x, y: pos.y + pt.y },
                                           3.0, c4, 4);
            }
        }

        ImDrawList_PopClipRect(draw_list);
    }
}


pub fn grid_canvas(size: &ImVec2, canvas: &mut GridCanvas) {
    unsafe {
        let io = igGetIO();
        let draw_list = igGetWindowDrawList();
        let pos = igGetCursorScreenPos_nonUDT2();
        let pos = ImVec2 { x: pos.x, y: pos.y };

        let c1 = igGetColorU32Vec4(ImVec4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 } );
        let c2 = igGetColorU32Vec4(ImVec4 { x: 0.2, y: 0.5, z: 0.95, w: 1.0 } );
        let c3 = igGetColorU32Vec4(ImVec4 { x: 1.0, y: 0.0, z: 1.0, w: 1.0 } );
        let c4 = igGetColorU32Vec4(ImVec4 { x: 0.8, y: 0.8, z: 0.8, w: 1.0 } );

        ImDrawList_AddRectFilled(draw_list,
                        pos, ImVec2 { x: pos.x + size.x, y: pos.y + size.y },
                        c1, 0.0, 0);
        igInvisibleButton(const_cstr!("grid_canvas").as_ptr(), *size);
        ImDrawList_PushClipRect(draw_list, pos, ImVec2 { x: pos.x + size.x, y: pos.y + size.y}, true);

        let pointer = (*io).MousePos;
        let pointer_incanvas = ImVec2 { x: pointer.x - pos.x, y: pointer.y - pos.y };
        let pointer_grid = canvas.screen_to_world(pointer_incanvas);

        let line = |c :ImU32,p1 :&ImVec2,p2 :&ImVec2| {
			ImDrawList_AddLine(draw_list,
				   ImVec2 { x: pos.x + p1.x, y: pos.y + p1.y },
				   ImVec2 { x: pos.x + p2.x, y: pos.y + p2.y },
				   c, 2.0);
        };

        // Drawing or adding line
        match (igIsItemHovered(0), igIsMouseDown(0), &mut canvas.adding_line) {
            (true, true, None) => {
                canvas.adding_line = Some(pointer_grid);
            },
            (_, false, Some(pt)) => {
                for l in GridCanvas::route_line(*pt, pointer_grid) {
                    canvas.lines.push(l);
                }
                canvas.adding_line = None;
            },
            _ => {},
        };

        // Draw permanent lines
        for (p1,p2) in &canvas.lines {
            line(c2, &canvas.world_to_screen(*p1), &canvas.world_to_screen(*p2));
        }

        // Draw temporary line
        if let Some(pt) = &canvas.adding_line {
            for (p1,p2) in GridCanvas::route_line(*pt, pointer_grid) {
                line(c3, &canvas.world_to_screen(p1), &canvas.world_to_screen(p2));
            }
        }

        // Draw grid + highlight on closest point if hovering?
        let (lo,hi) = canvas.points_in_view(*size);
        for x in lo.x..=hi.x {
            for y in lo.y..=hi.y {
                let pt = canvas.world_to_screen(Pt { x, y });
                ImDrawList_AddCircleFilled(draw_list, ImVec2 { x: pos.x + pt.x, y: pos.y + pt.y },
                                           3.0, c4, 4);
            }
        }

        ImDrawList_PopClipRect(draw_list);
    }
}

