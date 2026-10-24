#[cfg(target_os = "linux")]
fn main() {
    use ely_gpui_component::Assets;
    use elyterminal::{
        app,
        term::{Launch, Quote},
        workspace::{NewSession, Workspace},
    };
    use gpui::{
        EntityInputHandler, Focusable, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent,
        MouseMoveEvent, MouseUpEvent, PlatformInput, point, px,
    };
    use std::{fs, time::Duration};

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("elyterminal=info"))
        .init();
    let root = std::env::temp_dir().join(format!("ely-detach-{}", std::process::id()));
    fs::create_dir_all(root.join("live folder 中文")).unwrap();
    let program = r#"import os,sys,time,select,pathlib,urllib.parse
root=pathlib.Path(sys.argv[1]); (root/'boot').write_text(str(os.getpid()))
print('ORIGINAL OUTPUT MUST SURVIVE',flush=True)
while True:
 with (root/'heartbeat').open('a') as stream: stream.write('tick\n')
 if select.select([sys.stdin],[],[],0.05)[0]:
  line=sys.stdin.readline().strip()
  if line=='quit': break
  (root/'ack').write_text(line)
  if line=='movefolder':
   folder=root/'live folder 中文';os.chdir(folder)
   print('\x1b]7;file://'+urllib.parse.quote(str(folder))+'\x07',flush=True)
   print('\x1b]2;DETACHED_LIVE\x07',flush=True)
"#;
    gpui_platform::linux(gpui::WindowingModes::WAYLAND | gpui::WindowingModes::X11).with_assets(Assets).run(move |cx| {
        ely_gpui_component::init(cx).unwrap(); app::start(cx).unwrap();
        let launch=Launch {program:Some(("python3".into(),vec!["-u".into(),"-c".into(),program.into(),root.display().to_string()])),cwd:root.clone(),env:Vec::new(),reports:false,quote:Quote::Posix};
        let source=app::open(launch,cx).unwrap();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_millis(500)).await;
            let terminal=source.update(cx, |workspace,window,cx| {
                let terminal=workspace.sessions()[0].terminal.clone();
                workspace.rename(0,window,cx);
                let input=workspace.sessions()[0].editing.as_ref().unwrap().0.clone();
                input.update(cx,|input,cx| input.set_text("Live task".to_string(),cx));
                workspace.end_rename(terminal.entity_id(),true,window,cx);
                window.dispatch_action(Box::new(NewSession),cx);
                terminal
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(200)).await;
            let pid=cx.update(|cx| terminal.read(cx).pid());
            assert_eq!(fs::read_to_string(root.join("boot")).unwrap(),pid.to_string());
            let beats=fs::metadata(root.join("heartbeat")).unwrap().len();
            let target=source.update(cx,|workspace,window,cx| workspace.detach_session(terminal.entity_id(),point(px(60.),px(60.)),window,cx)).unwrap().unwrap();
            cx.background_executor().timer(Duration::from_millis(200)).await;
            source.update(cx,|workspace,_,_| assert_eq!(workspace.sessions().len(),1)).unwrap();
            target.update(cx,|workspace,window,cx| {
                assert_eq!(workspace.sessions().len(),1);
                assert_eq!(workspace.sessions()[0].terminal.entity_id(),terminal.entity_id());
                assert_eq!(workspace.sessions()[0].name,"Live task");
                assert_eq!(terminal.read(cx).pid(),pid);
                assert!(terminal.focus_handle(cx).is_focused(window));
                terminal.update(cx,|view,cx| view.replace_text_in_range(None,"movefolder\n",window,cx));
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(250)).await;
            assert!(fs::metadata(root.join("heartbeat")).unwrap().len()>beats);
            assert_eq!(fs::read_to_string(root.join("ack")).unwrap(),"movefolder");
            target.update(cx,|workspace,window,cx| {
                assert_eq!(workspace.sessions()[0].folder,root.join("live folder 中文"));
                assert_eq!(workspace.sessions()[0].title,"DETACHED_LIVE");
                terminal.update(cx,|view,cx| view.replace_text_in_range(None,"quit\n",window,cx));
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(200)).await;
            assert!(target.update(cx,|_,_,_|()).is_err());
            source.update(cx,|workspace,window,cx| {
                assert_eq!(workspace.sessions().len(),1);
                window.dispatch_action(Box::new(NewSession),cx);
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(200)).await;
            let reordered=source.update(cx,|workspace,_,_|workspace.sessions()[0].terminal.entity_id()).unwrap();
            gpui::AnyWindowHandle::from(source).update(cx,|_,window,cx| {
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent{position:point(px(80.),px(75.)),..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent{button:MouseButton::Left,position:point(px(80.),px(75.)),click_count:1,..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent{position:point(px(80.),px(140.)),pressed_button:Some(MouseButton::Left),..Default::default()}),cx);
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(120)).await;
            gpui::AnyWindowHandle::from(source).update(cx,|_,window,cx| {
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent{position:point(px(80.),px(150.)),pressed_button:Some(MouseButton::Left),..Default::default()}),cx);
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(120)).await;
            gpui::AnyWindowHandle::from(source).update(cx,|_,window,cx| {
                window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent{button:MouseButton::Left,position:point(px(80.),px(150.)),..Default::default()}),cx);
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(120)).await;
            source.update(cx,|workspace,_,_|assert_eq!(workspace.sessions()[1].terminal.entity_id(),reordered,"drag inside should reorder before detaching")).unwrap();
            let first=source.update(cx,|workspace,_,_|workspace.sessions()[0].terminal.clone()).unwrap();
            gpui::AnyWindowHandle::from(source).update(cx,|_,window,cx| {
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent{position:point(px(80.),px(75.)),..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent{button:MouseButton::Left,position:point(px(80.),px(75.)),click_count:1,..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent{position:point(px(170.),px(75.)),pressed_button:Some(MouseButton::Left),..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent{button:MouseButton::Left,position:point(px(170.),px(75.)),..Default::default()}),cx);
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(100)).await;
            assert_eq!(cx.update(|cx|cx.windows().into_iter().filter(|window|window.downcast::<Workspace>().is_some()).count()),1);
            gpui::AnyWindowHandle::from(source).update(cx,|_,window,cx| {
                window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent{button:MouseButton::Left,position:point(px(80.),px(75.)),click_count:1,..Default::default()}),cx);
                window.dispatch_event(PlatformInput::KeyDown(KeyDownEvent{keystroke:Keystroke::parse("escape").unwrap(),is_held:false,prefer_character_input:false}),cx);
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent{position:point(px(-30.),px(75.)),pressed_button:Some(MouseButton::Left),..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent{button:MouseButton::Left,position:point(px(-30.),px(75.)),..Default::default()}),cx);
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(100)).await;
            assert_eq!(cx.update(|cx|cx.windows().into_iter().filter(|window|window.downcast::<Workspace>().is_some()).count()),1);
            gpui::AnyWindowHandle::from(source).update(cx,|_,window,cx| {
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent{position:point(px(80.),px(75.)),..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent{button:MouseButton::Left,position:point(px(80.),px(75.)),click_count:1,..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent{position:point(px(-30.),px(75.)),pressed_button:Some(MouseButton::Left),..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent{button:MouseButton::Left,position:point(px(-30.),px(75.)),..Default::default()}),cx);
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(200)).await;
            assert!(!cx.update(|cx|cx.has_active_drag()),"detach clears the reorder drag");
            let targets=cx.update(|cx|cx.windows().into_iter().filter_map(|window|window.downcast::<Workspace>()).filter(|handle|*handle!=source).collect::<Vec<_>>());
            assert_eq!(targets.len(),1,"row drag should create one window");
            targets[0].update(cx,|workspace,_,_| assert_eq!(workspace.sessions()[0].terminal.entity_id(),first.entity_id())).unwrap();
            source.update(cx,|workspace,_,_| assert_eq!(workspace.sessions().len(),1)).unwrap();
            gpui::AnyWindowHandle::from(source).update(cx,|_,window,cx| {
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent{position:point(px(80.),px(75.)),..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent{button:MouseButton::Left,position:point(px(80.),px(75.)),click_count:1,..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent{position:point(px(-30.),px(75.)),pressed_button:Some(MouseButton::Left),..Default::default()}),cx);
                window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent{button:MouseButton::Left,position:point(px(-30.),px(75.)),..Default::default()}),cx);
            }).unwrap();
            assert_eq!(cx.update(|cx|cx.windows().into_iter().filter(|window|window.downcast::<Workspace>().is_some()).count()),2,"dragging the only session moves the existing window");
            let last=source.update(cx,|workspace,_,_|workspace.sessions()[0].terminal.entity_id()).unwrap();
            let final_window=source.update(cx,|workspace,window,cx|workspace.detach_session(last,point(px(120.),px(120.)),window,cx)).unwrap().unwrap();
            cx.background_executor().timer(Duration::from_millis(150)).await;
            assert!(source.update(cx,|_,_,_|()).is_err());
            final_window.update(cx,|workspace,_,_|assert_eq!(workspace.sessions()[0].terminal.entity_id(),last)).unwrap();
            println!("LIVE DETACH REGRESSION PASSED: same PID/entity, live task, name, folder/title events, input, exit routing, actual row reorder and detach, inside release, Escape, last-session transfer");
            cx.update(|cx|cx.quit());
        }).detach();
    });
}

#[cfg(not(target_os = "linux"))]
fn main() {}
