#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    thread,
    time::Duration,
};

use sysinfo::{ProcessesToUpdate, System};

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIcon,
    TrayIconBuilder,
};

use winit::{
    application::ApplicationHandler,
    event::{StartCause, WindowEvent},
    event_loop::{
        ActiveEventLoop,
        EventLoop,
        EventLoopProxy,
    },
    window::WindowId,
};


// ============================================================
// НАСТРОЙКИ
// ============================================================

/// Подстрока, которую ищем в имени процесса.
///
/// Например:
///
/// "agent"
///
/// Найдет:
///     agent.exe
///     my_agent.exe
///     AgentService.exe
///     test-agent-worker.exe
///
/// Поиск регистронезависимый.
const PROCESS_SUBSTRING: &str = "agent";


/// Процессы, которые нужно игнорировать.
///
/// Здесь указываются полные имена процессов, включая .exe.
///
/// Например:
///
/// "my_agent.exe"
/// "test_agent.exe"
///
/// Регистр значения не имеет.
const EXCLUDED_PROCESSES: &[&str] = &[
    "v4v_agent.exe",
    "klnagent.exe",
    "hvdagent.exe",
    "ssh-agent.exe",
];


/// Интервал проверки процессов в секундах.
const CHECK_INTERVAL_SECONDS: u64 = 7;


/// Название приложения.
const APP_NAME: &str = "Process Checker";


/// ID пункта "Выход" в меню Tray.
const EXIT_MENU_ID: &str = "exit";


// ============================================================
// СОБЫТИЯ ПРИЛОЖЕНИЯ
// ============================================================

#[derive(Debug)]
enum AppEvent {

    /// Найден подходящий процесс.
    ProcessStarted {
        name: String,
    },

    /// Пользователь выбрал "Выход".
    Exit,
}


// ============================================================
// ПОИСК ПОДХОДЯЩЕГО ПРОЦЕССА
// ============================================================

/// Возвращает имя первого процесса,
/// который соответствует условиям.
///
/// Условия:
///
/// 1. Имя процесса содержит PROCESS_SUBSTRING.
/// 2. Процесс не входит в EXCLUDED_PROCESSES.
///
/// Поиск регистронезависимый.
fn find_target_process(system: &System) -> Option<String> {

    // Приводим строку поиска к lowercase.
    let search_string =
        PROCESS_SUBSTRING.to_lowercase();


    for process in system.processes().values() {

        // Получаем имя процесса.
        let process_name =
            process.name().to_string_lossy().to_string();

        // Для сравнения используем lowercase.
        let process_name_lower =
            process_name.to_lowercase();


        // ----------------------------------------------------
        // Проверяем наличие искомой подстроки
        // ----------------------------------------------------

        if !process_name_lower.contains(&search_string) {
            continue;
        }


        // ----------------------------------------------------
        // Проверяем список исключений
        // ----------------------------------------------------

        let is_excluded =
            EXCLUDED_PROCESSES
                .iter()
                .any(|excluded| {

                    process_name_lower
                        == excluded.to_lowercase()
                });


        // Если процесс исключён — пропускаем.
        if is_excluded {
            continue;
        }


        // ----------------------------------------------------
        // Подходящий процесс найден
        // ----------------------------------------------------

        return Some(process_name);
    }


    // Ничего не найдено.
    None
}


// ============================================================
// WINDOWS MESSAGE BOX
// ============================================================

fn show_process_message(process_name: &str) {

    let title: Vec<u16> =
        format!("{APP_NAME}\0")
            .encode_utf16()
            .collect();


    let message: Vec<u16> =
        format!(
            "Обнаружен процесс:\n\n{}\n\nПоиск: \"{}\"",
            process_name,
            PROCESS_SUBSTRING
        )
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();


    unsafe {

        windows::Win32::UI::WindowsAndMessaging::MessageBoxW(

            None,

            windows::core::PCWSTR(
                message.as_ptr()
            ),

            windows::core::PCWSTR(
                title.as_ptr()
            ),

            windows::Win32::UI::WindowsAndMessaging::MB_OK
                |
            windows::Win32::UI::WindowsAndMessaging::MB_ICONWARNING,
        );
    }
}


// ============================================================
// ИКОНКА TRAY
// ============================================================

fn create_icon() -> tray_icon::Icon {

    let width: u32 = 32;
    let height: u32 = 32;


    let mut rgba =
        Vec::with_capacity(
            (width * height * 4) as usize
        );


    for y in 0..height {

        for x in 0..width {

            let border =
                x < 2
                || x >= width - 2
                || y < 2
                || y >= height - 2;


            if border {

                // Чёрная рамка.

                rgba.extend_from_slice(&[
                    0,
                    0,
                    0,
                    255,
                ]);

            } else {

                // Красный цвет.

                rgba.extend_from_slice(&[
                    220,
                    50,
                    50,
                    255,
                ]);
            }
        }
    }


    tray_icon::Icon::from_rgba(
        rgba,
        width,
        height,
    )
    .expect(
        "Не удалось создать иконку"
    )
}


// ============================================================
// APPLICATION
// ============================================================

struct App {

    /// Иконка приложения в системном трее.
    tray_icon: Option<TrayIcon>,


    /// Proxy для отправки событий
    /// из других потоков в EventLoop.
    proxy: EventLoopProxy<AppEvent>,


    /// Чтобы мониторинг был запущен только один раз.
    monitoring_started: bool,
}


// ============================================================
// APPLICATION HANDLER
// ============================================================

impl ApplicationHandler<AppEvent> for App {


    // ========================================================
    // RESUMED
    // ========================================================

    fn resumed(
        &mut self,
        _event_loop: &ActiveEventLoop,
    ) {

        // ----------------------------------------------------
        // Не запускаем повторно.
        // ----------------------------------------------------

        if self.monitoring_started {
            return;
        }


        self.monitoring_started = true;


        // ----------------------------------------------------
        // СОЗДАЁМ МЕНЮ
        // ----------------------------------------------------

        let menu = Menu::new();


        let exit_item =
            MenuItem::with_id(
                EXIT_MENU_ID,
                "Выход",
                true,
                None,
            );


        menu.append(&exit_item)
            .expect(
                "Не удалось создать меню"
            );


        // ----------------------------------------------------
        // СОЗДАЁМ ИКОНКУ
        // ----------------------------------------------------

        let icon =
            create_icon();


        // ----------------------------------------------------
        // СОЗДАЁМ TRAY
        // ----------------------------------------------------

        let tray =
            TrayIconBuilder::new()

                .with_menu(
                    Box::new(menu)
                )

                .with_tooltip(
                    APP_NAME
                )

                .with_icon(
                    icon
                )

                .build()

                .expect(
                    "Не удалось создать Tray Icon"
                );


        self.tray_icon =
            Some(tray);


        // ----------------------------------------------------
        // ОБРАБОТКА МЕНЮ TRAY
        // ----------------------------------------------------

        let proxy =
            self.proxy.clone();


        MenuEvent::set_event_handler(
            Some(
                move |event: MenuEvent| {

                    if event.id().0
                        == EXIT_MENU_ID
                    {

                        let _ =
                            proxy.send_event(
                                AppEvent::Exit
                            );
                    }
                },
            ),
        );


        // ----------------------------------------------------
        // ЗАПУСКАЕМ МОНИТОРИНГ
        // ----------------------------------------------------

        let proxy =
            self.proxy.clone();


        thread::spawn(move || {

            let mut system =
                System::new_all();


            // ------------------------------------------------
            // Был ли подходящий процесс
            // запущен на предыдущей проверке?
            // ------------------------------------------------

            let mut process_was_running =
                false;


            loop {

                // ------------------------------------------------
                // Обновляем список процессов.
                // ------------------------------------------------

                system.refresh_processes(
                    ProcessesToUpdate::All,
                    true,
                );


                // ------------------------------------------------
                // Ищем процесс.
                // ------------------------------------------------

                let found_process =
                    find_target_process(
                        &system
                    );


                let process_is_running =
                    found_process.is_some();


                // ------------------------------------------------
                // Процесс только что появился.
                // ------------------------------------------------

                if process_is_running
                    && !process_was_running
                {

                    if let Some(process_name) =
                        found_process
                    {

                        let _ =
                            proxy.send_event(
                                AppEvent::ProcessStarted {
                                    name: process_name,
                                }
                            );
                    }
                }


                // ------------------------------------------------
                // Запоминаем состояние.
                // ------------------------------------------------

                process_was_running =
                    process_is_running;


                // ------------------------------------------------
                // Ждём следующую проверку.
                // ------------------------------------------------

                thread::sleep(
                    Duration::from_secs(
                        CHECK_INTERVAL_SECONDS
                    )
                );
            }
        });
    }


    // ========================================================
    // USER EVENT
    // ========================================================

    fn user_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: AppEvent,
    ) {

        match event {


            // ------------------------------------------------
            // Найден процесс
            // ------------------------------------------------

            AppEvent::ProcessStarted {
                name,
            } => {

                show_process_message(
                    &name
                );
            }


            // ------------------------------------------------
            // Выход
            // ------------------------------------------------

            AppEvent::Exit => {

                // --------------------------------------------
                // Убираем обработчик меню.
                // --------------------------------------------

                MenuEvent::set_event_handler(
                    Option::<fn(MenuEvent)>::None
                );


                // --------------------------------------------
                // Удаляем Tray.
                // --------------------------------------------

                self.tray_icon =
                    None;


                // --------------------------------------------
                // Завершаем EventLoop.
                // --------------------------------------------

                event_loop.exit();
            }
        }
    }


    // ========================================================
    // WINDOW EVENT
    // ========================================================

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {

        // Окон у приложения нет.
    }


    // ========================================================
    // NEW EVENTS
    // ========================================================

    fn new_events(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _cause: StartCause,
    ) {

        // Ничего делать не нужно.
    }
}


// ============================================================
// MAIN
// ============================================================

fn main() {

    // --------------------------------------------------------
    // Создаём EventLoop.
    // --------------------------------------------------------

    let event_loop =
        EventLoop::<AppEvent>
            ::with_user_event()
            .build()
            .expect(
                "Не удалось создать EventLoop"
            );


    // --------------------------------------------------------
    // Proxy.
    // --------------------------------------------------------

    let proxy =
        event_loop.create_proxy();


    // --------------------------------------------------------
    // Создаём приложение.
    // --------------------------------------------------------

    let mut app =
        App {

            tray_icon: None,

            proxy,

            monitoring_started: false,
        };


    // --------------------------------------------------------
    // Запускаем EventLoop.
    // --------------------------------------------------------

    event_loop
        .run_app(&mut app)
        .expect(
            "Ошибка EventLoop"
        );
}