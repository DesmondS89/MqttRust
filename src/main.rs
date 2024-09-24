use anyhow::{self, Ok};

use embedded_svc::http::{client, server};
use embedded_svc::mqtt::client::Event;
use embedded_svc::{
    http::{Headers, Method},
    io::{Read, Write},
    wifi::{self, AuthMethod, ClientConfiguration, Configuration},
};
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::*;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::http::server::EspHttpServer;
use esp_idf_svc::mqtt::client::EspMqttEvent;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};
use esp_idf_svc::mqtt::client::{EspMqttConnection, QoS};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use heapless::String;
use log::*;
use serde::{Deserialize, Serialize};
use serde_urlencoded;
use std::str::FromStr;
use std::{thread::sleep, time::Duration};

// region variables
const DEBUG: bool = true;
const WIFI_SSID: &str = {
    if DEBUG {
        "Wokwi-GUEST"
    } else {
        "TP-LINK_2.4GHz_1A4A"
    }
};
const WIFI_PASSWORD: &str = "";
const MQTT_BROKER_URL: &str = "mqtt://broker.mqttdashboard.com:1883";
const MQTT_TOPIC: &str = "testtopic/1";

// pin
const BUTTON_PIN: u32 = 0;
const LED_PIN: u32 = 2;
// button state
const LED_ON: u32 = 1;
const LED_OFF: u32 = 0;
const BUTTON_PRESSED: u32 = 0;
const BUTTON_RELEASED: u32 = 1;
const BUTTON_DEBOUNCE_DELAY: u32 = 50;
const BUTTON_LONG_PRESS_DELAY: u32 = 1000;
const BUTTON_LONG_PRESSED: u32 = 2;
const BUTTON_SHORT_PRESSED: u32 = 3;
const BUTTON_LONG_PRESS: u32 = 4;
const BUTTON_SHORT_PRESS: u32 = 5;
const BUTTON_LONG_PRESS_RELEASE: u32 = 6;
const BUTTON_SHORT_PRESS_RELEASE: u32 = 7;
const BUTTON_LONG_PRESS_RELEASE_DELAY: u32 = 1000;
const BUTTON_SHORT_PRESS_RELEASE_DELAY: u32 = 50;
// delay
const DELAY: u64 = 1;

// endregion

#[derive(Deserialize)]
struct FormData {
    message: String<128>,
}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once.
    // Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly.
    // See https://github.com/esp-rs/esp-idf-template/issues/71
    // for more information.
    // Try to uncomment the line below and see if the build fails.
    esp_idf_sys::link_patches();
    // esp_idf_svc::log::EspLogger::initialize_default();
    // log::set_max_level(log::LevelFilter::Debug);

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // configurare wifi in modalità client
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    // connect_wifi(&mut wifi)?;
    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().unwrap(),
        // bssid: None,
        // auth_method: AuthMethod::WPA2Personal,
        auth_method: AuthMethod::None,
        password: WIFI_PASSWORD.try_into().unwrap(),
        channel: None,
        ..Default::default()
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    info!("Wifi started successfully");

    // Connect in dhcp mode with default timeout of 10 seconds
    wifi.connect()?;
    info!("Wifi connected");

    // Wait until the network interface is up with default timeout of 10 seconds
    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    // Print Out Wifi Connection Configuration
    while !wifi.is_connected().unwrap() {
        // Get and print connection configuration
        let config = wifi.get_configuration().unwrap();
        info!("Waiting for station {:?}", config);
    }

    info!("Wifi Connected");

    info!(
        "Connected Wi-Fi with WIFI_SSID `{}` and WIFI_PASS `{}`",
        WIFI_SSID, WIFI_PASSWORD
    );

    // Keep wifi running beyond when this function returns (forever)
    // Do not call this if you ever want to stop or access it later.
    // Otherwise it should be returned from this function and kept somewhere
    // so it does not go out of scope.
    // https://doc.rust-lang.org/stable/core/mem/fn.forget.html
    // core::mem::forget(wifi);

    let server_configuration = esp_idf_svc::http::server::Configuration {
        http_port: 80,
        ..Default::default()
    };

    let mut server = EspHttpServer::new(&server_configuration)?;

    info!("Server started");

    // Create MQTT Connection
    info!("Creating MQTT Connection");

    // Create MQTT Client Configuration
    // connect_mqtt()?;

    // Create MQTT Client Configuration
    let mqtt_configuration = MqttClientConfiguration {
        client_id: "esp32".try_into().unwrap(),
        ..Default::default()
    };

    let (mut client, mut connection) = EspMqttClient::new(MQTT_BROKER_URL, &mqtt_configuration)?;

    // Connect to MQTT Broker
    // connection.connect()?;
    let message = b"Hello from ESP32";
    client.publish(MQTT_TOPIC, QoS::AtLeastOnce, false, message)?;

    // Subscribe to MQTT Topic
    client.subscribe(MQTT_TOPIC, QoS::AtLeastOnce)?;

    server.fn_handler("/", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(create_html_response().as_bytes())
            .map(|_| ())
    })?;

    // add route for publishing message to mqtt broker when the data is sent from the form
    server.fn_handler("/", Method::Post, move |mut req| -> anyhow::Result<()> {
        let mut body = Vec::new();
        req.read(&mut body)?;

        let mut body_string: String<1024> = String::new();
        body_string
            .push_str(std::str::from_utf8(&body).unwrap())
            .unwrap();

        req.into_ok_response()?;
        let form_data: FormData = serde_urlencoded::from_bytes(&body).unwrap();
        let message = form_data.message;

        let (mut client, mut connection) =
            EspMqttClient::new(MQTT_BROKER_URL, &mqtt_configuration)?;
        client.publish(MQTT_TOPIC, QoS::AtLeastOnce, false, message.as_bytes())?;

        Ok(())
    })?;
    info!("Server routes added");
    // core::mem::forget(server);

    // define the pin driver for the led and button by their respective pin numbers defined in the constants
    let mut led = PinDriver::output(peripherals.pins.gpio2)?;
    let mut button = PinDriver::input(peripherals.pins.gpio0)?;

    // let mut led = PinDriver::output(peripherals.pins.gpio4)?;
    // let mut button = PinDriver::input(peripherals.pins.gpio9)?;
    button.set_pull(Pull::Down)?;

    // Main task no longer needed, free up some memory
    let mut button_state = BUTTON_RELEASED;
    let mut button_press_time = 0;
    loop {
       
        // we are using thread::sleep here to make sure the watchdog isn't triggered
        FreeRtos::delay_ms(10);
        // check if the button is pressed and I want to use the debounce delay to make sure the button is pressed
        // for a certain amount of time before I consider it as pressed
        if button.is_low() {
            // if the button is pressed, I want to check if the button state is released
            if button_state == BUTTON_RELEASED {
                // if the button state is released, I want to set the button state to pressed
                button_state = BUTTON_PRESSED;
                // I want to set the button press time to the current time
                button_press_time = unsafe { esp_idf_sys::esp_timer_get_time() } / 1000;
            }
            // if the button state is pressed, I want to check if the button has been pressed for a long time
            if button_state == BUTTON_PRESSED
                && (unsafe { (esp_idf_sys::esp_timer_get_time() / 1000) - button_press_time })
                    > (BUTTON_LONG_PRESS_DELAY as i64)
            {
                // if the button has been pressed for a long time, I want to set the button state to long pressed
                button_state = BUTTON_LONG_PRESSED;
                // I want to set the led to on
                led.set_high()?;
                // I want to publish a message to the mqtt broker
                client.publish(MQTT_TOPIC, QoS::AtLeastOnce, false, b"Button Long Pressed")?;
            }
        } else {
            // if the button is released, I want to check if the button state is pressed
            if button_state == BUTTON_PRESSED {
                // if the button state is pressed, I want to set the button state to released
                button_state = BUTTON_RELEASED;
                // I want to set the led to off
                led.set_low()?;
                // I want to publish a message to the mqtt broker
                client.publish(MQTT_TOPIC, QoS::AtLeastOnce, false, b"Button Pressed")?;
            }
            // if the button state is long pressed, I want to set the button state to released
            if button_state == BUTTON_LONG_PRESSED {
                button_state = BUTTON_RELEASED;
            }
        }

        let duration = Duration::from_secs(DELAY);
        sleep(duration);
    }
}

// load the html form to be displayed on the web page when the server is accessed via a browser or client application
fn create_html_response() -> String<1024> {
    // the html page is in a external file
    let html = include_str!("index.html");
    let mut html_string: String<1024> = String::new();
    html_string.push_str(html).unwrap();
    return html_string;
}

// fn create_server() -> anyhow::Result<EspHttpServer<'static>> {
//     let peripherals = Peripherals::take().unwrap();
//     let sys_loop = EspSystemEventLoop::take()?;
//     let nvs = EspDefaultNvsPartition::take()?;

//     // configurare wifi in modalità client
//     let mut wifi = BlockingWifi::wrap(
//         EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
//         sys_loop,
//     )?;

//     connect_wifi(&mut wifi)?;

//     info!(
//         "Connected Wi-Fi with WIFI_SSID `{}` and WIFI_PASS `{}`",
//         WIFI_SSID, WIFI_PASSWORD
//     );

//     let server_configuration = esp_idf_svc::http::server::Configuration {
//         ..Default::default()
//     };

//     // Keep wifi running beyond when this function returns (forever)
//     // Do not call this if you ever want to stop or access it later.
//     // Otherwise it should be returned from this function and kept somewhere
//     // so it does not go out of scope.
//     // https://doc.rust-lang.org/stable/core/mem/fn.forget.html
//     core::mem::forget(wifi);

//     Ok(EspHttpServer::new(&server_configuration)?)
// }

// Connect to Wi-Fi network with the provided SSID and password. This function is blocking.
// It will return once the Wi-Fi connection is established. If the connection fails, an error will be returned.
// The Wi-Fi connection is established using the provided `wifi` instance.
// The Wi-Fi configuration is set to client mode with the provided SSID and password.
// The Wi-Fi connection is started, and the connection is established.
// The network interface is then brought up.
// If the connection is successful, the function will return `Ok(())`.
// If the connection fails, an error will be returned.
// fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
//     let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
//         ssid: WIFI_SSID.try_into().unwrap(),
//         // bssid: None,
//         // auth_method: AuthMethod::WPA2Personal,
//         auth_method: AuthMethod::None,
//         password: WIFI_PASSWORD.try_into().unwrap(),
//         channel: None,
//         ..Default::default()
//     });

//     wifi.set_configuration(&wifi_configuration)?;

//     wifi.start()?;
//     info!("Wifi started successfully");

//     // Connect in dhcp mode with default timeout of 10 seconds
//     wifi.connect()?;
//     info!("Wifi connected");

//     // Wait until the network interface is up with default timeout of 10 seconds
//     wifi.wait_netif_up()?;
//     info!("Wifi netif up");

//     // Print Out Wifi Connection Configuration
//     while !wifi.is_connected().unwrap() {
//         // Get and print connection configuration
//         let config = wifi.get_configuration().unwrap();
//         info!("Waiting for station {:?}", config);
//     }

//     info!("Wifi Connected");

//     Ok(())
// }

// fn connect_mqtt() -> anyhow::Result<()> {
//     let peripherals = Peripherals::take().unwrap();
//     let sys_loop = EspSystemEventLoop::take()?;
//     let nvs = EspDefaultNvsPartition::take()?;

//     Ok(())
// }
