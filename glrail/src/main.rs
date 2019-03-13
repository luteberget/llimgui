use std::ffi::{CString, CStr};
use std::ptr;
use const_cstr::const_cstr;
use imgui_sys_bindgen::sys::*;

mod sdlinput;

// Domain
mod app;
mod command_builder;
mod schematic;
mod selection;

use self::app::*;
use self::command_builder::*;
use self::selection::*;

pub fn entity_to_string(id :EntityId, inf :&Infrastructure) -> String {
  match inf.get(id) {
      Some(Entity::Track(ref t)) => {
          format!("{:#?}", t)
      },
      Some(Entity::Node(p,ref n)) => {
          format!("Node at {}: {:#?}", p,n)
      },
      Some(Entity::Object(ref o)) => {
          format!("Id: {}", id)
      },
      _ => { format!("Error id={} not found.", id) }
  }
}

use imgui_sys_bindgen::sys::ImVec2;
pub fn world2screen(topleft: ImVec2, bottomright: ImVec2, center :(f64,f64), zoom: f64, pt :(f32,f32)) -> ImVec2 {
    let scale = if bottomright.x - topleft.x < bottomright.y - topleft.y {
        (bottomright.x-topleft.x) as f64 / zoom
    } else {
        (bottomright.y-topleft.y) as f64 / zoom
    };
    let x = 0.5*(topleft.x + bottomright.x) as f64 + scale*(pt.0 as f64  - center.0);
    let y = 0.5*(topleft.y + bottomright.y) as f64 + scale*(-(pt.1 as f64 -  center.1));
    ImVec2 {x: x as _ , y: y as _ }
}

pub fn screen2world(topleft: ImVec2, bottomright: ImVec2, center: (f64, f64), zoom: f64, pt:ImVec2) -> (f32,f32) {
    let scale = if bottomright.x - topleft.x < bottomright.y - topleft.y {
        (bottomright.x-topleft.x) as f64 / zoom
    } else {
        (bottomright.y-topleft.y) as f64 / zoom
    };
    // mousex = 0.5 tlx + 0.5 brx + scale*ptx - scale*cx
    // ptx = 1/scale*(mousex - 0.5tlx - 0.5brx + scale*cx)
    let x = 1.0/scale*(pt.x as f64 - (0.5*(topleft.x + bottomright.x)) as f64) + center.0;
    let y = 1.0/scale*(pt.y as f64 - (0.5*(topleft.y + bottomright.y)) as f64) + center.1;
    (x as _,(-y) as _ )
}

pub fn screen2worldlength(topleft: ImVec2, bottomright: ImVec2, zoom: f64, d :f32) -> f32 {
    let scale = if bottomright.x - topleft.x < bottomright.y - topleft.y {
        (bottomright.x-topleft.x) as f64 / zoom
    } else {
        (bottomright.y-topleft.y) as f64 / zoom
    };

    ((d as f64)/scale) as f32
}

pub fn  line_closest_pt(a :&ImVec2, b :&ImVec2, p :&ImVec2) -> ImVec2 {
    let ap = ImVec2{ x: p.x - a.x, y:  p.y - a.y};
    let ab_dir = ImVec2 { x: b.x - a.x, y: b.y - a.y };
    let dot = ap.x * ab_dir.x + ap.y * ab_dir.y;
    if dot < 0.0 { return *a; }
    let ab_len_sqr = ab_dir.x * ab_dir.x + ab_dir.y * ab_dir.y;
    if dot > ab_len_sqr { return *b; }
    let ac = ImVec2{ x: ab_dir.x * dot / ab_len_sqr, y: ab_dir.y * dot / ab_len_sqr } ;
    ImVec2 { x : a.x + ac.x, y: a.y + ac.y }
}

pub fn dist2(a :&ImVec2, b :&ImVec2) -> f32 { 
    (a.x - b.x)*(a.x - b.x) + (a.y - b.y)*(a.y - b.y)
}

pub struct OpenObject {
    pub newkey :String,
    pub open_subobjects :Vec<(String, Box<OpenObject>)>,
}

pub fn input_text_string(
    label: &CStr,
    hint: Option<&CStr>,
    buffer: &mut String,
    flags: ImGuiInputTextFlags) {
    buffer.push('\0');
    input_text(label,hint, unsafe { buffer.as_mut_vec() },flags);
    buffer.pop();
}

pub fn input_text(
    label: &CStr,
    hint: Option<&CStr>,
    buffer: &mut Vec<u8>,
    mut flags: ImGuiInputTextFlags) {

   unsafe extern "C" fn resize_func(data: *mut ImGuiInputTextCallbackData) -> std::os::raw::c_int  {
       //println!("BufTextLen {:?}", (*data).BufTextLen);
       let vecptr = ((*data).UserData as *mut Vec<u8>);
       (*vecptr).resize((*data).BufTextLen as usize + 1, '\0' as u8);
       (*vecptr)[(*data).BufTextLen as usize ] = '\0' as u8;
       (*data).Buf = (*vecptr).as_mut_ptr() as _;
       0
   }

   match hint {
       Some(hint) => {
           unsafe {
           igInputTextWithHint(
               label.as_ptr(),
                //const_cstr!("").as_ptr(),
                //const_cstr!("New key").as_ptr(),
               hint.as_ptr(),
                buffer.as_mut_ptr() as _,
                buffer.capacity()+1,
                flags | (ImGuiInputTextFlags__ImGuiInputTextFlags_CallbackResize as ImGuiInputTextFlags) ,
                Some(resize_func),
                buffer as *mut _ as _);
           }
       },
       None => {
           // TODO igInputText
           unimplemented!()
       }
   }

}

pub fn show_text(s :&str) {
    unsafe {
    igTextSlice(s.as_ptr() as _ , s.as_ptr().offset(s.len() as _ ) as _ );
    }
}


type UserData = serde_json::Map<String, serde_json::Value>;

pub fn json_editor(types: &[*const i8; 6], data :&mut UserData, open :&mut OpenObject) {
    let v2_0 = ImVec2 { x: 0.0, y : 0.0 };
    unsafe {
        use imgui_sys_bindgen::sys::*;
        let mut del = None;
        for (i,(k,v)) in data.iter_mut().enumerate() {
            igPushIDInt(i as _);
            show_text(k);

            if igButton(const_cstr!("\u{f056}").as_ptr(), v2_0) {
                del = Some(k.clone());
            }
            igSameLine(0.0, -1.0);
            
            igPushItemWidth(3.0*16.0);

            let l_null = const_cstr!("null");
            let l_bool = const_cstr!("bool");
            let l_number = const_cstr!("num");
            let l_text = const_cstr!("text");
            let l_array = const_cstr!("arr");
            let l_object = const_cstr!("obj");

            let curr_type_str = match v {
                             serde_json::Value::Null => l_null,
                             serde_json::Value::Bool(_) => l_bool,
                             serde_json::Value::Number(_) => l_number,
                             serde_json::Value::String(_) => l_text,
                             serde_json::Value::Object(_) => l_object,
                             serde_json::Value::Array(_) => l_array,
                             _ => l_text,
                         };

            if igBeginCombo(const_cstr!("##type").as_ptr(), curr_type_str.as_ptr(),
                         ImGuiComboFlags__ImGuiComboFlags_NoArrowButton as _) {

                if igSelectable(l_null.as_ptr(), l_null == curr_type_str, 0, v2_0) 
                    && l_null != curr_type_str {
                        *v = serde_json::Value::Null;
                }
                if igSelectable(l_bool.as_ptr(), l_bool == curr_type_str, 0, v2_0) 
                    && l_bool != curr_type_str {
                        *v = serde_json::Value::Bool(Default::default());
                }
                if igSelectable(l_number.as_ptr(), l_number == curr_type_str, 0, v2_0) 
                    && l_number != curr_type_str {
                        *v = serde_json::Value::Number(serde_json::Number::from_f64(0.0).unwrap());
                }
                if igSelectable(l_text.as_ptr(), l_text == curr_type_str, 0, v2_0) 
                    && l_text != curr_type_str {
                        *v = serde_json::Value::String(Default::default());
                }
                if igSelectable(l_array.as_ptr(), l_array == curr_type_str, 0, v2_0) 
                    && l_array != curr_type_str {
                        *v = serde_json::Value::Array(Default::default());
                }
                if igSelectable(l_object.as_ptr(), l_object == curr_type_str, 0, v2_0) 
                    && l_object != curr_type_str {
                        *v = serde_json::Value::Object(Default::default());
                }
                igEndCombo();
            }
            igPopItemWidth();

            igPushItemWidth(-1.0);

            match v {
                serde_json::Value::Null => {},
                serde_json::Value::Bool(ref mut b) => {
                    let l_true = const_cstr!("true");
                    let l_false = const_cstr!("false");
                    igSameLine(0.0, -1.0);
                    if igBeginCombo(const_cstr!("##bool").as_ptr(), 
                                    (if *b { l_true } else { l_false }).as_ptr(),0) {

                        if igSelectable(l_false.as_ptr(), !*b, 0, v2_0) && *b {
                            *b = false;
                        }
                        if igSelectable(l_true.as_ptr(), *b, 0, v2_0) && !*b {
                            *b = true;
                        }
                        igEndCombo();
                    }
                },
                serde_json::Value::Number(ref mut n) => {
                    let mut num : f32 = n.as_f64().unwrap() as _;
                    igSameLine(0.0, -1.0);
                    igInputFloat(const_cstr!("##num").as_ptr(), 
                                 &mut num as *mut _, 0.0, 1.0, 
                                 const_cstr!("%g").as_ptr(), 0);
                    if igIsItemDeactivatedAfterEdit() {
                        *n = serde_json::Number::from_f64(num as _).unwrap();
                    }
                },
                serde_json::Value::String(ref mut s) => {
                    igSameLine(0.0, -1.0);
                    input_text_string(
                        const_cstr!("##text").as_cstr(), 
                        Some(const_cstr!("empty").as_cstr()), 
                        s, 0);
                },
                serde_json::Value::Array(ref mut a) => {
                    igSameLine(0.0, -1.0);
                    if igTreeNodeStr(const_cstr!("Array").as_ptr()) {
                        igText(const_cstr!("...").as_ptr());
                        igTreePop();
                    }
                },
                serde_json::Value::Object(ref mut o) => {
                    igSameLine(0.0, -1.0);
                    if igTreeNodeStr(const_cstr!("Object").as_ptr()) {

                        //json_editor
                        json_editor(&types, o, open);

                        igTreePop();
                    }
                },
                _ => unimplemented!(),
            }

            igPopItemWidth();
            //println!("{:?}: {:?}", k,v);
            igPopID();
        }

        if let Some(k) = del {
            data.remove(&k);
        }

        if igButton(const_cstr!("\u{f055}").as_ptr(), ImVec2 { x: 0.0, y: 0.0 })  {
            use std::mem;
            let s = &mut open.newkey;
            if s.len() > 0 {
                data.insert(
                    mem::replace(s, String::new()),
                    serde_json::Value::Null);
            }
        }

       igSameLine(0.0, -1.0);
       input_text_string( const_cstr!("##newkey").as_cstr(), 
                   Some(const_cstr!("New key").as_cstr()), &mut open.newkey, 0);
    }
}



fn gui_init() {
    use imgui_sys_bindgen::sys::*;
    use std::ptr;
    unsafe {
        let _ig = igCreateContext(ptr::null_mut());
        let _io = igGetIO();
        igStyleColorsDark(ptr::null_mut());
    }
}

//fn gui_frame() {
//        let io = igGetIO();
//        igNewFrame();
//        //igRender();
//}

fn gui_destroy() {
}

pub fn wake() {
    unsafe {
        use std::ptr;
        use sdl2::sys::*;

        let ev = SDL_UserEvent { 
            type_: SDL_EventType::SDL_USEREVENT as _, 
            timestamp: sdl2::sys::SDL_GetTicks(),
            windowID: 0,
            code: 0,
            data1: ptr::null_mut(),
            data2: ptr::null_mut(),
        };

        let mut ev = SDL_Event { user: ev };
        SDL_PushEvent(&mut ev as _);
    }
}

fn main() -> Result<(), String>{
    use log::LevelFilter;
    simple_logging::log_to_stderr(LevelFilter::Debug);

    let json_types: [*const i8; 6] = [
        const_cstr!("Null").as_ptr(),
        const_cstr!("Bool").as_ptr(),
        const_cstr!("Num").as_ptr(),
        const_cstr!("Text").as_ptr(),
        const_cstr!("Obj").as_ptr(),
        const_cstr!("Arr").as_ptr(),
    ];


    let mut app = app::App::new();
    //let mut action_queue = Vec::new();

    let sdl_context = sdl2::init()?;
    let event_subsystem = sdl_context.event()?;
    let video_subsystem = sdl_context.video()?;
    let window = video_subsystem
        .window("glrail", 800, 600)
        .opengl()
        .resizable()
        .position_centered()
        .build()
        .map_err(|e| format!("{}", e))?;

    let _gl_context = window.gl_create_context().expect("Couldn't create GL context");
    gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as _);


    let mut canvas = window.into_canvas()
        .target_texture()
        .present_vsync()
        .build()
        .map_err(|e| format!("{}", e))?;

        //let mut ev = SDL_Event { type_: SDL_EventType::SDL_USEREVENT as _, user: ev };
    //println!("Using SDL_Renderer \"{}\"", canvas.info().name);
    //canvas.set_draw_color(sdl2::pixels::Color::RGB(255, 0, 0));
    //canvas.clear();
    //canvas.present();

    let texture_creator : sdl2::render::TextureCreator<_> 
        = canvas.texture_creator();

    gui_init();
    let io = unsafe { imgui_sys_bindgen::sys::igGetIO() };

    unsafe {
            use imgui_sys_bindgen::sys::*;
        //    //io.Fonts->AddFontFromFileTTF("../../misc/fonts/Roboto-Medium.ttf", 16.0f);

		//      //ImVector<ImWchar> ranges;
        //    let ranges = ImVector_ImWchar_ImVector_ImWchar();
		//      //ImFontGlyphRangesBuilder builder;
        //    let builder = ImFontGlyphRangesBuilder_ImFontGlyphRangesBuilder();
        //    ImFontGlyphRangesBuilder_AddText(builder, black_left.as_ptr(), ptr::null());
        //    ImFontGlyphRangesBuilder_AddText(builder, black_right.as_ptr(), ptr::null());
        //    //
        //    //builder.AddRanges(io.Fonts->GetGlyphRangesJapanese()); // Add one of the default ranges
        //    //ImFontGlyphRangesBuilder_AddRanges( builder, ImFontAtlas_GetGlyphRangesJapanese((*io).Fonts));
        //    ImFontGlyphRangesBuilder_AddRanges( builder, ImFontAtlas_GetGlyphRangesDefault((*io).Fonts));

		//    //builder.BuildRanges(&ranges);                          // Build the final result (ordered ranges with all the unique characters submitted)
        //    ImFontGlyphRangesBuilder_BuildRanges(builder, ranges);

		//    //io.Fonts->AddFontFromFileTTF("myfontfile.ttf", size_in_pixels, NULL, ranges.Data);
		//    //io.Fonts->Build();                                     // Build the atlas while 'ranges' is still in scope and not deleted.


        //    let fconfig = ptr::null();
        //    //let franges = ptr::null();
        //    ImFontAtlas_AddFontFromFileTTF((*io).Fonts, 
        //           const_cstr!("DejaVuSansMono.ttf").as_ptr(),
        //           22.0, fconfig, (*ranges).Data);
        //    ImFontAtlas_Build((*io).Fonts);

        
        igStyleColorsLight(ptr::null_mut());
        ImFontAtlas_AddFontFromFileTTF((*io).Fonts, 
        //       //const_cstr!("DejaVuSansMono.ttf").as_ptr(),
               const_cstr!("Roboto-Medium.ttf").as_ptr(),
               16.0, ptr::null(), ptr::null());
        //ImFontAtlas_AddFontDefault((*io).Fonts, ptr::null());

        let config = ImFontConfig_ImFontConfig();
        (*config).MergeMode = true;
        (*config).GlyphMinAdvanceX = 16.0;
        let ranges : [std::os::raw::c_ushort;3] = [0xf000, 0xf82f, 0x0];
        //#define ICON_MIN_FA 0xf000
        //#define ICON_MAX_FA 0xf82f

        ImFontAtlas_AddFontFromFileTTF((*io).Fonts,
            const_cstr!("fa-solid-900.ttf").as_ptr(),
            14.0,  config, &ranges as _ );

        ImFontAtlas_Build((*io).Fonts);
    }

    let mut imgui_renderer = imgui_sys_opengl::Renderer::new(|s| video_subsystem.gl_get_proc_address(s) as _);
    let mut imgui_sdl = sdlinput::ImguiSdl2::new();




    use sdl2::event::Event;
    fn not_mousemotion(ev :&Event) -> bool {
        if let &Event::MouseMotion { .. } = ev { false } else { true }
    }
    fn exit_on(ev :&Event) -> bool {
        if let &Event::Quit { .. } = ev { true } else { false }
    }

    fn app_event(ev :&Event, app :&mut App, command_input :bool, canvas_input :bool) {
        //println!("app event {:?}");
        match ev {
            Event::TextInput { ref text, .. } => {
                for chr in text.chars() {
                    if chr == ',' {
                        if app.view.command_builder.is_none() {
                            app.main_menu();
                        }
                    }
                    if chr == '.' {
                        if app.view.command_builder.is_none() {
                            if let Some(screen) = app.context_menu() {
                                app.view.command_builder = Some(CommandBuilder::new_screen(screen));
                            }
                        }
                    }
                }
            }
            _ => {},
        }
        if canvas_input {
            use sdl2::keyboard::{Keycode, Mod};
            let ctrl_mod = Mod::LCTRLMOD | Mod::RCTRLMOD;
            let shift_mod = Mod::LSHIFTMOD | Mod::RSHIFTMOD;
            match ev {
                Event::KeyDown { keycode: Some(ref keycode), keymod, .. } => {
                    println!("canvas {:?}", keycode);
                    match keycode {
                        Keycode::Left | Keycode::H => {
                            if keymod.intersects(ctrl_mod) {
                                app.move_view(InputDir::Left);
                            } else {
                                app.move_selection(InputDir::Left);
                            }
                        },
                        Keycode::Right | Keycode::L => {
                            if keymod.intersects(ctrl_mod) {
                                app.move_view(InputDir::Right);
                            } else {
                                app.move_selection(InputDir::Right);
                            }
                        },
                        Keycode::Up | Keycode::K => {
                            if keymod.intersects(ctrl_mod) {
                                app.move_view(InputDir::Up);
                            } else {
                                app.move_selection(InputDir::Up);
                            }
                        },
                        Keycode::Down | Keycode::J => {
                            if keymod.intersects(ctrl_mod) {
                                app.move_view(InputDir::Down);
                            } else {
                                app.move_selection(InputDir::Down);
                            }
                        },
                        _ => {},
                    }
                },
                _ => {},
            }
        }

        if command_input {
            let mut new_screen_func = None;
            if let Some(cb) = &mut app.view.command_builder {
                if let CommandScreen::Menu(Menu { choices }) = cb.current_screen() {
                    for (c,_,f) in choices {
                        match ev {
                            Event::TextInput { ref text, .. } => {
                                for chr in text.chars() {
                                    if chr == *c {
                                        new_screen_func = Some(*f);
                                    }
                                }
                            }
                            _ => {},
                        }
                    }
                }
            }

            if let Some(f) = new_screen_func {
                if let Some(s) = f(app) {
                    if let Some(ref mut c) = app.view.command_builder {
                        c.push_screen(s);
                    }
                } else {
                    app.view.command_builder = None;
                }
            }
        }
    }

    //let win1 = CString::new("sidebar1").unwrap();

    unsafe {
        use imgui_sys_bindgen::sys::*;
        //(*imgui_sys_bindgen::sys::igGetIO()).IniFilename = ptr::null_mut();
        (*igGetIO()).ConfigFlags |= ImGuiConfigFlags__ImGuiConfigFlags_NavEnableKeyboard as i32;

        //igMayaStyle();
        //CherryTheme();
    }

    let mut user_data = serde_json::json!({});

    let mut open_object : OpenObject = OpenObject { 
        newkey: String::new(),
        open_subobjects: Vec::new(),
    };

    let mut sidebar_size :f32 = 200.0;
    let mut issues_size :f32 = 200.0;
    let canvas_bg = 60 + (60<<8) + (60<<16) + (255<<24);
    let line_col  = 208 + (208<<8) + (175<<16) + (255<<24);
    let selected_col  = 175 + (175<<8) + (255<<16) + (255<<24);
    let line_hover_col  = 255 + (50<<8) + (50<<16) + (255<<24);
    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut i :i64 = 0;
    let mut capture_command_key = false;
    let mut capture_canvas_key = false;

    let mut events = |mut f: Box<FnMut(sdl2::event::Event) -> bool>| {
        'running: loop {
            let mut render = false;
              let event =  event_pump.wait_event();
              imgui_sdl.handle_event(&event);
              if exit_on(&event) { break 'running; }
              if not_mousemotion(&event) { render = true; }
              app_event(&event, &mut app, capture_command_key, capture_canvas_key);

              for event2 in event_pump.poll_iter() {
                  imgui_sdl.handle_event(&event2);
                  if exit_on(&event2) { break 'running; }
                  if not_mousemotion(&event2) { render = true; }
                  app_event(&event2, &mut app, capture_command_key, capture_canvas_key);
              }

            for _ in 1..=3 {
              for event2 in event_pump.poll_iter() {
                  imgui_sdl.handle_event(&event2);
                  if exit_on(&event2) { break 'running; }
                  app_event(&event2, &mut app, capture_command_key, capture_canvas_key);
              }

              let c = sdl2::pixels::Color::RGB(15,15,15);
              //println!("frame! color {:?}", c);
              canvas.set_draw_color(c);
              canvas.clear();
              //gui_frame();

              imgui_sdl.frame(&canvas.window(), &event_pump.mouse_state());

              // TODO move this out of main loop
                let caret_right = const_cstr!("\u{f0da}");
                let caret_left = const_cstr!("\u{f0d9}");
                let (caret_left_halfsize,caret_right_halfsize) = unsafe {
                    let mut l = igCalcTextSize(caret_left.as_ptr(), ptr::null(), false, -1.0);
                    let mut r = igCalcTextSize(caret_right.as_ptr(), ptr::null(), false, -1.0);
                    l.x *= 0.5; l.y *= 0.5; r.x *= 0.5; r.y *= 0.5;
                    (l,r)
                };

              use self::app::*;
              use imgui_sys_bindgen::sys::*;
              let v2_0 = ImVec2 { x: 0.0, y: 0.0 };
              let small = ImVec2 { x: 200.0, y: 200.0 };

              // Check for updates from all background threads
              app.update();

              //if let Derive::Ok(Schematic { pos_map, .. }) = &app.model.inf.schematic {
              //    println!("pos_map:  {:?}", pos_map);
              //}

              unsafe {
                  if app.view.show_imgui_demo {
                      igShowDemoWindow(&mut app.view.show_imgui_demo as *mut bool);
                  }

                  let mouse_pos = (*io).MousePos;

                  let viewport = igGetMainViewport();
                  igSetNextWindowPos((*viewport).Pos, ImGuiCond__ImGuiCond_Always as _, v2_0);
                  igSetNextWindowSize((*viewport).Size, ImGuiCond__ImGuiCond_Always as _ );
                  let dockspace_window_flags = ImGuiWindowFlags__ImGuiWindowFlags_NoTitleBar
                      | ImGuiWindowFlags__ImGuiWindowFlags_NoCollapse
                      | ImGuiWindowFlags__ImGuiWindowFlags_NoResize
                      | ImGuiWindowFlags__ImGuiWindowFlags_NoMove
                      | ImGuiWindowFlags__ImGuiWindowFlags_NoBringToFrontOnFocus
                      | ImGuiWindowFlags__ImGuiWindowFlags_NoNavFocus;

                  igBegin(const_cstr!("Root").as_ptr(), ptr::null_mut(), dockspace_window_flags as _ );
                  
                  let mut root_size = igGetContentRegionAvail();
                  let mut main_size = ImVec2 { x: root_size.x - sidebar_size, ..root_size };

                  igSplitter(true, 2.0, &mut sidebar_size as _, &mut main_size.x as _, 100.0, 100.0, -1.0);

                  igBeginChild(const_cstr!("Sidebar").as_ptr(), ImVec2 { x: sidebar_size, y: root_size.y } , false,0);

                  // Start new command
                    if igButton(const_cstr!("\u{f044}").as_ptr(), ImVec2 { x: 0.0, y: 0.0 })  {
                        app.main_menu();
                    }

                    //igSameLine(0.0,-1.0);

//                  match app.view.command_builder {
//                      None => igText(const_cstr!("App default state.").as_ptr()),
//                      Some(CommandBuilder::MainMenu) => igText(const_cstr!("Main menu").as_ptr()),
//                      Some(CommandBuilder::JoinTwo) => igText(const_cstr!("Select two points for joining.").as_ptr()),
//                      Some(CommandBuilder::JoinOne(_)) => igText(const_cstr!("Select one more point for joining.").as_ptr()),
//                  }
//

                  
                  if igCollapsingHeader(const_cstr!("All objects").as_ptr(),
                                        ImGuiTreeNodeFlags__ImGuiTreeNodeFlags_DefaultOpen as _ ) {
                      for (i,e) in app.model.inf.entities.iter().enumerate() {
                          match e {
                              Some(Entity::Track(_))  => { 
                                  let s = CString::new(format!("Track##{}", i)).unwrap();
                                  if igSelectable(s.as_ptr(),
                                                  app.view.selection == Selection::Object(i), 0, v2_0) {
                                      //println!("SET {}", i);
                                      app.view.selection = Selection::Object(i);
                                  }
                              },
                              Some(Entity::Node(p,_))   => { 
                                  let s = CString::new(format!("Node @ {}##{}", p,i)).unwrap();
                                  if igSelectable(s.as_ptr(), 
                    
                              app.view.selection == Selection::Object(i), 0, v2_0) {
                                      //println!("SET NODE {}", i);
                                      app.view.selection = Selection::Object(i);
                                  }
                              },
                              Some(Entity::Object(_)) => { 
                                  igText(const_cstr!("Object#0").as_ptr()); 
                              },
                              _ => {},
                          }
                      }
                  }

                  if igCollapsingHeader(const_cstr!("Object properties").as_ptr(),
                                        ImGuiTreeNodeFlags__ImGuiTreeNodeFlags_DefaultOpen as _ ) {
                      match &app.view.selection {
                          Selection::Object(id) => {
                              let s = entity_to_string(*id, &app.model.inf);
                              show_text(&s);
                          },
                          _ => {
                              igText(const_cstr!("No object selected.").as_ptr());
                          }
                      }
                  }


                  if igCollapsingHeader(const_cstr!("Routes").as_ptr(),
                                        ImGuiTreeNodeFlags__ImGuiTreeNodeFlags_DefaultOpen as _ ) {
                      for r in &app.model.routes {

                      }
                  }
                  if igCollapsingHeader(const_cstr!("Scenarios").as_ptr(),
                                        ImGuiTreeNodeFlags__ImGuiTreeNodeFlags_DefaultOpen as _ ) {
                      for r in &app.model.scenarios {

                      }
                  }

                  if igCollapsingHeader(const_cstr!("User data editor").as_ptr(),
                                        ImGuiTreeNodeFlags__ImGuiTreeNodeFlags_DefaultOpen as _ ) {
                      json_editor(&json_types, user_data.as_object_mut().unwrap(), &mut open_object);
                  }

                  igEndChild();
                  igSameLine(0.0, -1.0);
                  igBeginChild(const_cstr!("CanvasandIssues").as_ptr(), main_size, false, 0);

                  let mut mainmain_size = ImVec2 { y: main_size.y - issues_size, ..main_size };
                  igSplitter(false, 2.0, &mut mainmain_size.y as _, &mut issues_size as _, 100.0, 100.0, -1.0);

                  // CANVAS!

                  igBeginChild(const_cstr!("Canvas").as_ptr(), mainmain_size, false, 0);
                  capture_canvas_key = igIsWindowFocused(0);

                  let draw_list = igGetWindowDrawList();
                  //igText(const_cstr!("Here is the canvas:").as_ptr());

                  match &app.model.inf.schematic {
                      Derive::Wait => {
                          igText(const_cstr!("Solving...").as_ptr());
                      },
                      Derive::Error(ref e) => {
                          let s = CString::new(format!("Error: {}", e)).unwrap();
                          igText(s.as_ptr());
                      },
                      Derive::Ok(ref s) => {
                          let mut hovered_item = None;
                          let canvas_pos = igGetCursorScreenPos();
                          let mut canvas_size = igGetContentRegionAvail();
                          let canvas_lower = ImVec2 { x: canvas_pos.x + canvas_size.x,
                                                      y: canvas_pos.y + canvas_size.y };
                          if canvas_size.x < 10.0 { canvas_size.x = 10.0 }

                          if canvas_size.y < 10.0 { canvas_size.y = 10.0 }
                          ImDrawList_AddRectFilled(draw_list, canvas_pos,
                                                   ImVec2 { x: canvas_pos.x + canvas_size.x,
                                                            y: canvas_pos.y + canvas_size.y, },
                                                            canvas_bg,
                                                    0.0, 0);
                          let clicked = igInvisibleButton(const_cstr!("canvasbtn").as_ptr(), canvas_size);
                          let right_clicked = igIsItemHovered(0) && igIsMouseClicked(1,false);
                          let canvas_hovered = igIsItemHovered(0);

                          let (center,zoom) = app.view.viewport;

                          if igIsItemActive() && igIsMouseDragging(0,-1.0) {
                              (app.view.viewport.0).0 -= screen2worldlength(canvas_pos, canvas_lower, zoom, (*io).MouseDelta.x) as f64;
                              (app.view.viewport.0).1 += screen2worldlength(canvas_pos, canvas_lower, zoom, (*io).MouseDelta.y) as f64;
                          }

                          if igIsItemHovered(0) {
                              let wheel = (*io).MouseWheel;
                              //println!("{}", wheel);
                              let wheel2 = 1.0-0.2*(*io).MouseWheel;
                              //println!("{}", wheel2);
                              (app.view.viewport.1) *= wheel2 as f64;
                          }
                          

                          // Iterate the schematic 


                          ImDrawList_PushClipRect(draw_list, canvas_pos, canvas_lower, true);

                          let mut lowest = std::f32::INFINITY;

                          for (k,v) in &s.lines {
                              //println!("{:?}, {:?}", k,v);
                              let mut hovered = false;
                              let selected = if let Selection::Object(id) = &app.view.selection { id == k } else { false };
                              for i in 0..(v.len()-1) {
                                  let p1 = world2screen(canvas_pos, canvas_lower, center, zoom, v[i]);
                                  let p2 = world2screen(canvas_pos, canvas_lower, center, zoom, v[i+1]);
                                  let hovered = dist2(&mouse_pos, &line_closest_pt(&p1, &p2, &mouse_pos)) < 100.0;
                                  if hovered {
                                      hovered_item = Some(*k);
                                  }
                                  ImDrawList_AddLine(draw_list, p1, p2, 
                                                     if selected { selected_col }
                                                     else if canvas_hovered && hovered { line_hover_col } else { line_col }, 2.0);
                                  lowest = lowest.min(v[i].1);
                                  lowest = lowest.min(v[i+1].1);
                              }
                          }
                          for (k,v) in &s.points {
                              let mut p = world2screen(canvas_pos, canvas_lower, center, zoom, *v);
                              let tl = ImVec2 { x: p.x - caret_right_halfsize.x, 
                                                 y: p.y - caret_right_halfsize.y };
                              let br = ImVec2 { x: p.x + caret_right_halfsize.x, 
                                                 y: p.y + caret_right_halfsize.y };

                              lowest = lowest.min(v.1);
                              let selected = if let Selection::Object(id) = &app.view.selection { id == k } else { false };
                              let hover = igIsMouseHoveringRect(tl,br,false);
                              ImDrawList_AddText(draw_list, tl, 
                                                 if selected { selected_col } 
                                                 else if canvas_hovered && hover { line_hover_col } else { line_col }, 
                                                 caret_right.as_ptr(), ptr::null());
                              if hover {
                                  hovered_item = Some(*k);
                              }
                          }

                          let (mut last_x,mut line_no) = (None,0);
                          for (x,_id,pos) in &s.pos_map {
                              let x = *x;
                              // TODO use line_no to calculate number of text heights to lower
                              //println!("{:?}", lowest);
                              ImDrawList_AddLine(draw_list,
                                                 world2screen(canvas_pos, canvas_lower, center, zoom, (x, lowest - 0.5)),
                                                 world2screen(canvas_pos, canvas_lower, center, zoom, (x, lowest - 0.5 - (line_no+1) as f32)),
                                                 line_col, 1.0);
                              if Some(x) == last_x {
                                  line_no += 1;
                              } else {
                                  line_no = 0;
                              }
                              let s = CString::new(format!(" {}", pos)).unwrap();
                              ImDrawList_AddText(draw_list, 
                                                 world2screen(canvas_pos, canvas_lower, center, zoom, (x, lowest - 0.5 - (line_no) as f32)),
                                                 line_col,
                                                 s.as_ptr(), ptr::null());
                              last_x = Some(x);
                          }

                          if let Selection::Pos(pos, y, id) = &app.view.selection {
                              if let Some(x) = s.find_pos(*pos) {
                                  //println!("Drawing at {:?} {:?}", x, y);
                                ImDrawList_AddLine(draw_list, 
                                   world2screen(canvas_pos, canvas_lower, center, zoom, (x, y - 0.25)),
                                   world2screen(canvas_pos, canvas_lower, center, zoom, (x, y + 0.25)),
                                   selected_col, 2.0);
                              }
                          }

                          ImDrawList_PopClipRect(draw_list);

                          if clicked {
                              app.clicked_object(hovered_item, 
                                                 screen2world(canvas_pos, canvas_lower, center, zoom, (*io).MousePos));
                          }

                          if right_clicked {
                                if let Some(screen) = app.context_menu() {
                                    app.view.command_builder = Some(CommandBuilder::new_screen(screen));
                                }
                          }

                          if let Some(id) = hovered_item {
                              if canvas_hovered {
                                  igBeginTooltip();
                                  show_text(&entity_to_string(id, &app.model.inf));
                                  igEndTooltip();
                              }
                          }

                      },
                  }

                  igEndChild();


                  igBeginChild(const_cstr!("Issues").as_ptr(),ImVec2 { x: main_size.x, y: issues_size } ,false,0);
                  igText(const_cstr!("Here are the issues:").as_ptr());
                  for error in &app.model.errors {

                  }
                  igEndChild();



                  igEndChild();

                  igEnd();



                  let mut overlay_start = || {
                      igSetNextWindowBgAlpha(0.75);
                      igSetNextWindowPos(ImVec2 { x: sidebar_size, y: 0.0 },
                      //igSetNextWindowPos((*viewport).Pos, 
                         ImGuiCond__ImGuiCond_Always as _, v2_0);
                      igPushStyleColor(ImGuiCol__ImGuiCol_TitleBgActive as _, 
                                     ImVec4 { x: 1.0, y: 0.65, z: 0.7, w: 1.0 });
                      igBegin(const_cstr!("Command").as_ptr(), ptr::null_mut(),
                        (ImGuiWindowFlags__ImGuiWindowFlags_AlwaysAutoResize | 
                        ImGuiWindowFlags__ImGuiWindowFlags_NoMove | 
                        ImGuiWindowFlags__ImGuiWindowFlags_NoResize) as _
                        );

                      capture_command_key = igIsWindowFocused(0);
                  };
                      
                  let overlay_end = || {
                      igEnd();
                      igPopStyleColor(1);
                  };
                  
                  // Overlay command builder
                  let mut new_screen_func = None;
                  let mut alb_execute = false;
                  let mut alb_cancel = false;
                  if let Some(ref mut command_builder) = &mut app.view.command_builder {
                      match command_builder.current_screen() {
                          CommandScreen::Menu(Menu { choices }) => {
                              // Draw menu
                              //
                              overlay_start();

                              for (i,c) in choices.iter().enumerate() {
                                igPushIDInt(i as _);
                                  if igSelectable(const_cstr!("##mnuitm").as_ptr(), false, 0, v2_0) {
                                      new_screen_func = Some(c.2);
                                  }

                                  igSameLine(0.0, -1.0);

                                  let s = CString::new(format!("{} - ", c.0)).unwrap();
                                  igTextColored( ImVec4 { x: 0.95, y: 0.5, z: 0.55, w: 1.0 }, s.as_ptr());

                                  igSameLine(0.0, -1.0);
                                  //igText(const_cstr!("context").as_ptr());
                                  show_text(&c.1);
                                igPopID();
                              }

                              overlay_end();

                          },
                          CommandScreen::ArgumentList(alb) => {
                              overlay_start();
                              for (i,(name, status, arg)) in alb.arguments.iter_mut().enumerate() {
                                  igPushIDInt(i as _);

                                  let s = CString::new(name.as_str()).unwrap();
                                  match status {
                                      ArgStatus::Done => {
                                          let c = ImVec4 { x: 0.55, y: 0.55, z: 0.80, w: 1.0 };
                                          igTextColored(c, s.as_ptr());
                                          igSameLine(0.0,-1.0);
                                          match arg {
                                              Arg::Id(Some(x)) => {
                                                  show_text(&format!("obj:{}", x));
                                              },
                                              Arg::Float(val) => {
                                                  show_text(&format!("{}", val));
                                              },
                                              _ => { panic!(); },
                                          }
                                      },
                                      ArgStatus::NotDone => {
                                          let c = ImVec4 { x: 0.95, y: 0.5,  z: 0.55, w: 1.0 };
                                          igTextColored(c, s.as_ptr());
                                          igSameLine(0.0,-1.0);
                                          match arg {
                                              Arg::Id(x) => {
                                                  show_text(&format!("obj:{:?}", x));
                                              },
                                              Arg::Float(ref mut val) => {
                                                igInputFloat(const_cstr!("##num").as_ptr(), 
                                                             val as *mut _, 0.0, 1.0, 
                                                             const_cstr!("%g").as_ptr(), 0);
                                              },
                                          }
                                      },
                                  };

                                  igPopID();
                              }

                              if igButton(const_cstr!("\u{f04b} Execute").as_ptr(), v2_0) {
                                  alb_execute = true;
                              }

                              igSameLine(0.0,-1.0);
                              if igButton(const_cstr!("\u{f05e} Cancel").as_ptr(), v2_0) {
                                  alb_cancel = true;
                              }
                              overlay_end();
                          },
                          _ => {},
                      }
                  }

                  if let Some(f) = new_screen_func {
                      if let Some(s) = f(&mut app) {
                          if let Some(ref mut c) = app.view.command_builder {
                              c.push_screen(s);
                          }
                      } else {
                          app.view.command_builder = None;
                      }
                  }

                  if alb_execute {
                      use std::mem;
                      let cb = mem::replace(&mut app.view.command_builder, None);
                      if let Some(cb) = cb {
                          cb.execute(&mut app);
                      }
                  }

                  if alb_cancel {
                      app.view.command_builder = None;
                  }

              }

              imgui_renderer.render();
              canvas.present();


              if app.view.want_to_quit {
                  break 'running;
              }
            }
        }
    };

    events(Box::new(|ev| {
        use sdl2::event::Event;
        use sdl2::keyboard::Keycode;
        match ev {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    return true;
                },

                _ => {},

        }

        return false;

    }));

    gui_destroy();

    Ok(())
}