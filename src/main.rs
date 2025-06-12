use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuItem, NSStatusBar,
    NSVariableStatusItemLength,
};
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSString, NSAutoreleasePool, NSData, NSSize, NSPoint, NSRect};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use sysinfo::{System};
use base64::{engine::general_purpose, Engine as _};

mod login_item;

struct AppState {
    status_item: id,
    metrics: Arc<Mutex<SystemMetrics>>,
    cpu_icon: id,
    ram_icon: id,
}

#[derive(Clone, Default)]
struct SystemMetrics {
    cpu_usage: String,
    ram_usage: String,
}

extern "C" fn toggle_login_item(this: &Object, _cmd: Sel, sender: id) {
    unsafe {
        let is_enabled = login_item::is_enabled();
        let new_state = !is_enabled;
        login_item::set_enabled(new_state);

        let _: () = msg_send![sender, setState: if new_state { 1 } else { 0 }];
    }
}

fn create_image_from_base64_svg(base64_data_uri: &str) -> id {
    unsafe {
        let b64_data = base64_data_uri.split(',').last().unwrap_or("");
        let svg_bytes = general_purpose::STANDARD.decode(b64_data).unwrap_or_default();
        let ns_data = NSData::dataWithBytes_length_(nil, svg_bytes.as_ptr() as *const c_void, svg_bytes.len() as u64);
        let image: id = msg_send![class!(NSImage), alloc];
        let image: id = msg_send![image, initWithData: ns_data];
        let size = NSSize::new(16.0, 16.0);
        let _: () = msg_send![image, setSize: size];
        let _: () = msg_send![image, setTemplate:YES];
        image
    }
}

fn create_attributed_title(metrics: &SystemMetrics, cpu_icon: id, ram_icon: id) -> id {
    unsafe {
        let final_str: id = msg_send![class!(NSMutableAttributedString), new];
        let attachment_bounds = NSRect::new(NSPoint::new(0.0, -2.5), NSSize::new(16.0, 16.0));

        let cpu_attachment: id = msg_send![class!(NSTextAttachment), new];
        let _: () = msg_send![cpu_attachment, setImage: cpu_icon];
        let _: () = msg_send![cpu_attachment, setBounds: attachment_bounds];
        let cpu_icon_str: id = msg_send![class!(NSAttributedString), attributedStringWithAttachment: cpu_attachment];
        let _: () = msg_send![final_str, appendAttributedString: cpu_icon_str];

        let cpu_text_str = make_nsstring(&format!(" {}  ", metrics.cpu_usage));
        let cpu_text_attr: id = msg_send![class!(NSAttributedString), alloc];
        let _: () = msg_send![cpu_text_attr, initWithString: cpu_text_str];
        let _: () = msg_send![final_str, appendAttributedString: cpu_text_attr];

        let ram_attachment: id = msg_send![class!(NSTextAttachment), new];
        let _: () = msg_send![ram_attachment, setImage: ram_icon];
        let _: () = msg_send![ram_attachment, setBounds: attachment_bounds];
        let ram_icon_str: id = msg_send![class!(NSAttributedString), attributedStringWithAttachment: ram_attachment];
        let _: () = msg_send![final_str, appendAttributedString: ram_icon_str];
        
        let ram_text_str = make_nsstring(&format!(" {}", metrics.ram_usage));
        let ram_text_attr: id = msg_send![class!(NSAttributedString), alloc];
        let _: () = msg_send![ram_text_attr, initWithString: ram_text_str];
        let _: () = msg_send![final_str, appendAttributedString: ram_text_attr];

        final_str
    }
}

fn create_simple_title(metrics: &SystemMetrics) -> id {
    let title_text = format!("🖥️ {} | 💾 {}", metrics.cpu_usage, metrics.ram_usage);
    make_nsstring(&title_text)
}

extern "C" fn update_title(this: &Object, _cmd: Sel, _timer: id) {
    unsafe {
        let app_state_ptr: *mut c_void = *this.get_ivar("appState");
        if app_state_ptr.is_null() { return; }
        let app_state = &*(app_state_ptr as *mut AppState);
        let metrics = app_state.metrics.lock().unwrap().clone();
        
        if app_state.cpu_icon != nil && app_state.ram_icon != nil {
            let attributed_title = create_attributed_title(&metrics, app_state.cpu_icon, app_state.ram_icon);
            let button: id = msg_send![app_state.status_item, button];
            let _: () = msg_send![button, setAttributedTitle: attributed_title];
        } else {
            let title = create_simple_title(&metrics);
            let button: id = msg_send![app_state.status_item, button];
            let _: () = msg_send![button, setTitle: title];
        }
    }
}

fn make_nsstring(s: &str) -> id {
    unsafe { NSString::alloc(nil).init_str(s) }
}

fn update_cpu_loop(metrics: Arc<Mutex<SystemMetrics>>) {
    thread::spawn(move || {
        let mut sys = System::new();
        loop {
            sys.refresh_cpu();
            let usage = sys.global_cpu_info().cpu_usage();
            let mut data = metrics.lock().unwrap();
            data.cpu_usage = format!("{:.0}%", usage);
            drop(data);
            thread::sleep(Duration::from_millis(1200));
        }
    });
}

fn get_macos_compressed_memory() -> Option<u64> {
    let output = Command::new("vm_stat").output().ok()?;
    let output_str = String::from_utf8(output.stdout).ok()?;
    let mut page_size: Option<u64> = None;
    let mut pages_occupied: Option<u64> = None;

    if let Some(first_line) = output_str.lines().next() {
        if let Some(start) = first_line.find('(') {
            if let Some(end) = first_line.find(" bytes)") {
                page_size = first_line[start + 1..end].parse::<u64>().ok();
            }
        }
    }

    for line in output_str.lines() {
        if line.starts_with("Pages occupied by compressor:") {
            pages_occupied = line.split_whitespace().last()?.parse::<u64>().ok();
            break;
        }
    }
    if let (Some(size), Some(pages)) = (page_size, pages_occupied) { Some(size * pages) } else { None }
}

fn update_ram_loop(metrics: Arc<Mutex<SystemMetrics>>) {
    thread::spawn(move || {
        let mut sys = System::new_all();
        loop {
            sys.refresh_memory();
            let mut used_memory_b = sys.used_memory();
            if cfg!(target_os = "macos") {
                if let Some(compressed_memory) = get_macos_compressed_memory() {
                    used_memory_b += compressed_memory;
                }
            }
            let used_gb = used_memory_b as f64 / 1024.0_f64.powi(3);
            let mem_str = format!("{:.1}GB", used_gb);
            let mut data = metrics.lock().unwrap();
            data.ram_usage = mem_str;
            drop(data);
            thread::sleep(Duration::from_millis(2100));
        }
    });
}

fn create_app_menu(target: id) -> id {
    unsafe {
        let menu = NSMenu::new(nil).autorelease();

        // "Start at Login" menu item
        let login_title = make_nsstring("Start at Login");
        let login_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            login_title,
            sel!(toggleLoginItem:),
            make_nsstring(""),
        );
        let _: () = msg_send![login_item, setTarget: target];
        
        // Set initial checkmark state
        if login_item::is_enabled() {
            let _: () = msg_send![login_item, setState: 1];
        } else {
            let _: () = msg_send![login_item, setState: 0];
        }
        menu.addItem_(login_item);

        // Separator
        let sep = NSMenuItem::separatorItem(nil);
        menu.addItem_(sep);

        // "Quit" menu item
        let quit_title = make_nsstring("Quit");
        let q_key = make_nsstring("q");
        let quit_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            quit_title,
            sel!(terminate:),
            q_key,
        );
        menu.addItem_(quit_item);
        menu
    }
}

fn main() {
    let cpu_icon_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20v2"/><path d="M12 2v2"/><path d="M17 20v2"/><path d="M17 2v2"/><path d="M2 12h2"/><path d="M2 17h2"/><path d="M2 7h2"/><path d="M20 12h2"/><path d="M20 17h2"/><path d="M20 7h2"/><path d="M7 20v2"/><path d="M7 2v2"/><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="8" y="8" width="8" height="8" rx="1"/></svg>"#;
    let ram_icon_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 19v-3"/><path d="M10 19v-3"/><path d="M14 19v-3"/><path d="M18 19v-3"/><path d="M8 11V9"/><path d="M16 11V9"/><path d="M12 11V9"/><path d="M2 15h20"/><path d="M2 7a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v1.1a2 2 0 0 0 0 3.837V17a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-5.1a2 2 0 0 0 0-3.837Z"/></svg>"#;

    let cpu_icon_b64 = format!("data:image/svg+xml;base64,{}", general_purpose::STANDARD.encode(cpu_icon_svg.as_bytes()));
    let ram_icon_b64 = format!("data:image/svg+xml;base64,{}", general_purpose::STANDARD.encode(ram_icon_svg.as_bytes()));
    
    let shared_metrics = Arc::new(Mutex::new(SystemMetrics::default()));
    update_cpu_loop(Arc::clone(&shared_metrics));
    update_ram_loop(Arc::clone(&shared_metrics));

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory);

        let status_item = NSStatusBar::systemStatusBar(nil).statusItemWithLength_(NSVariableStatusItemLength);

        let cpu_icon = create_image_from_base64_svg(&cpu_icon_b64);
        let ram_icon = create_image_from_base64_svg(&ram_icon_b64);

        let mut app_state = Box::new(AppState {
            status_item,
            metrics: Arc::clone(&shared_metrics),
            cpu_icon,
            ram_icon,
        });

        {
            let data = app_state.metrics.lock().unwrap();
            if app_state.cpu_icon != nil && app_state.ram_icon != nil {
                let initial_title = create_attributed_title(&data, app_state.cpu_icon, app_state.ram_icon);
                let button: id = msg_send![status_item, button];
                let _: () = msg_send![button, setAttributedTitle: initial_title];
            } else {
                let initial_title = create_simple_title(&data);
                let button: id = msg_send![status_item, button];
                let _: () = msg_send![button, setTitle: initial_title];
            }
        }
        
        let mut timer_decl = ClassDecl::new("PulseTimerTarget", class!(NSObject)).unwrap();
        timer_decl.add_ivar::<*mut c_void>("appState");
        timer_decl.add_method(sel!(updateTitle:), update_title as extern "C" fn(&Object, Sel, id));
        let timer_target_class = timer_decl.register();
        let timer_target: id = msg_send![timer_target_class, new];
        (*timer_target).set_ivar("appState", &mut *app_state as *mut _ as *mut c_void);

        let timer: id = msg_send![class!(NSTimer), timerWithTimeInterval:0.5 target:timer_target selector:sel!(updateTitle:) userInfo:nil repeats:YES];
        let run_loop: id = msg_send![class!(NSRunLoop), mainRunLoop];
        let common_modes = make_nsstring("kCFRunLoopCommonModes");
        let _: () = msg_send![run_loop, addTimer: timer forMode: common_modes];
        
        let mut login_item_decl = ClassDecl::new("ToggleLoginItemTarget", class!(NSObject)).unwrap();
        login_item_decl.add_method(sel!(toggleLoginItem:), toggle_login_item as extern "C" fn(&Object, Sel, id));
        let login_item_target_class = login_item_decl.register();
        let login_item_target: id = msg_send![login_item_target_class, new];

        let menu = create_app_menu(login_item_target);
        let _: () = msg_send![status_item, setMenu: menu];

        app.run();
    }
}
