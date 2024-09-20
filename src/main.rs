use actix_web::{get, App, HttpResponse, HttpServer, Responder};
use anyhow;
use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};
use esp_idf_svc::mqtt::client::{EspMqttConnection, QoS};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use heapless::String;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::{thread::sleep, time::Duration};

#[derive(Serialize, Deserialize)]
struct ApiResponse {
    status: u16,
    message: heapless::String<64>,
}

// region variables
const WIFI_SSID: &str = "D-Link-2BE3C3";
const WIFI_PASSWORD: &str = "K3RazVjHpN";
const MQTT_BROKER_URL: &str = "mqtt://6804c12a8e254e5c9f0d45b0ea9c0b2a.s1.eu.hivemq.cloud:8883";
const MQTT_PORT: u16 = 8883;
const MQTT_TOPIC: &str = "testtopic/1";
const MQTT_CLIENT_ID: &str = "esp32";
const MQTT_USERNAME: &str = "hivemq.webclient.1726606709307";
const MQTT_PASSWORD: &str = "9wD6c7YmrP<tIi!5VL#>";

// endregion

#[actix_web::main]
async fn main() -> std::io::Result() {
    // It is necessary to call this function once.
    // Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly.
    // See https://github.com/esp-rs/esp-idf-template/issues/71
    // for more information.
    // Try to uncomment the line below and see if the build fails.
    esp_idf_sys::link_patches();

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // Initializes a mutable `wifi` instance by wrapping an `EspWifi` object with `BlockingWifi`.
    //
    // The `EspWifi` object is created using the provided modem peripheral, system event loop,
    // and an optional non-volatile storage (NVS) reference. The `BlockingWifi::wrap` function
    // is then used to create a blocking Wi-Fi instance, which is assigned to `wifi`.
    //
    // # Parameters
    // - `peripherals.modem`: The modem peripheral used for Wi-Fi.
    // - `sysloop`: The system event loop used for handling Wi-Fi events.
    // - `nvs`: An optional reference to non-volatile storage (NVS).
    //
    // # Returns
    // A result containing the initialized `wifi` instance or an error if the initialization fails.
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
    )?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: heapless::String::<32>::from_str(WIFI_SSID).unwrap(),
        bssid: None,
        auth_method: AuthMethod::None,
        password: heapless::String::<64>::try_from(WIFI_PASSWORD).unwrap(),
        channel: None,
        scan_method: todo!(),
        pmf_cfg: todo!(),
    }))?;

    // Start Wifi
    wifi.start()?;

    // Connect Wifi
    wifi.connect()?;

    // Wait until the network interface is up
    wifi.wait_netif_up()?;

    // Print Out Wifi Connection Configuration
    while !wifi.is_connected().unwrap() {
        // Get and print connection configuration
        let config = wifi.get_configuration().unwrap();
        println!("Waiting for station {:?}", config);
    }

    println!("Wifi Connected");

    // Wait for IP Address
    while wifi.is_connected().unwrap_or(false) {
        println!("Waiting for IP Address");
        sleep(Duration::from_secs(1));
    }

    // println!("IP Address: {:?}", wifi.get_ip_address()?);

    // Create a Web Client
    let mqtt_config = MqttClientConfiguration {
        client_id: Some(MQTT_CLIENT_ID.into()),
        username: Some(MQTT_USERNAME.into()),
        password: Some(MQTT_PASSWORD.into()),
        keep_alive_interval: Some(Duration::from_secs(60)),
        protocol_version: Default::default(),
        connection_refresh_interval: Duration::from_secs(30),
        reconnect_timeout: None,
        network_timeout: Duration::from_secs(30),
        lwt: None,
        disable_clean_session: false,
        task_prio: 1, // Set a valid priority value
        task_stack: 4096,
        buffer_size: 1024,
        out_buffer_size: 1024,
        use_global_ca_store: false,
        skip_cert_common_name_check: false,
        crt_bundle_attach: None,
        server_certificate: None,
        client_certificate: None,
        private_key: None,
        private_key_password: None,
    };

    let (mut client, mut connection) = EspMqttClient::new(MQTT_BROKER_URL, &mqtt_config)?;
    // Create API Response
    let response = crate::ApiResponse {
        status: 200,
        message: heapless::String::<64>::from_str("Hello World").unwrap(),
    };

    // Create MQTT Client Configuration
    let mqtt_config = MqttClientConfiguration {
        client_id: Some(MQTT_CLIENT_ID.into()),
        username: Some(MQTT_USERNAME.into()),
        password: Some(MQTT_PASSWORD.into()),
        keep_alive_interval: Some(Duration::from_secs(60)),
        ..Default::default()
    };

    // Create MQTT Client
    let (mut client, mut connection) = EspMqttClient::new(MQTT_BROKER_URL, &mqtt_config)?;

    // Subscribe to MQTT Topic
    client.subscribe(MQTT_TOPIC.into(), QoS::AtLeastOnce)?;

    // Publish to MQTT Topic
    client.publish(
        MQTT_TOPIC.into(),
        QoS::AtLeastOnce,
        false,
        "Hello World".as_bytes(),
    )?;

    // Event mqtt loop
    // while let Some(event) = connection.next() {
    //     match event {
    //         Err(e) => {
    //             println!("MQTT Error: {:?}", e);
    //         }
    //         Ok(event) => match event {
    //             Event::Connected(_) => {
    //                 println!("Connected to MQTT Broker");
    //             }
    //             Event::Publish(publish) => {
    //                 println!(
    //                     "Received message on topic '{}': {}",
    //                     publish.topic, publish.payload
    //                 );
    //             }
    //             Event::Disconnected(_) => {
    //                 println!("Disconnected from MQTT Broker");
    //             }
    //             _ => {}
    //         },
    //     }
    // }

    // Event Loop
    // while let Some(event) = connection.next() {
    //     match event {
    //         Err(e) => {
    //             println!("MQTT Error: {:?}", e);
    //         }
    //         Ok(event) => match event {
    //             Event::Connected(_) => {
    //                 println!("Connected to MQTT Broker");
    //             }
    //             Event::Publish(publish) => {
    //                 println!(
    //                     "Received message on topic '{}': {}",
    //                     publish.topic, publish.payload
    //                 );
    //             }
    //             Event::Disconnected(_) => {
    //                 println!("Disconnected from MQTT Broker");
    //             }
    //             _ => {}
    //         },
    //     }
    // }

    HttpServer::new(|| App::new().service(hello))
        .bind(("localhost", 5001))?
        .run()
        .await?;

    loop {
        // Keep waking up device to avoid watchdog reset
        sleep(Duration::from_millis(1000));
    }
}

// crea elenco di endpoint per il server web
// fn create_endpoints() -> Vec<ApiEndpoint> {
//     return vec![
//         ApiEndpoint {
//             method: HttpMethod::Get,

//         },
//         ApiEndpoint {
//             method: HttpMethod::Post,
//         },
//     ]
// }

// Configura il server web

fn configure_webserver() {
    // Create a new instance of the `EspWebServer` struct.
    // The `EspWebServer` struct is created using the provided system event loop and
    // an optional non-volatile storage (NVS) reference.
    // The `EspWebServer::new` function is then used to create a new instance of the `EspWebServer` struct.
    // The created instance is assigned to the `server` variable.
    let server = EspWebServer::new(sysloop.clone(), Some(nvs))?;

    // Creo un endpoint per il server web
    let endpoint = ApiEndpoint {
        method: HttpMethod::Get,
    };

    // Register the endpoint with the server
    // The `register` method is called on the `server` instance to register the `endpoint` with the server.
    // The `register` method returns a result containing the registered endpoint or an error if the registration fails.
    // The registered endpoint is assigned to the `endpoint` variable.
    let endpoint = server.register(endpoint)?;

    // Set the handler for the endpoint
    // The `set_handler` method is called on the `endpoint` instance to set the handler for the endpoint.
    // The `set_handler` method takes a closure as an argument, which is used to define the handler logic.
    // The handler logic is defined as a closure that takes a request and returns a response.
    // The `set_handler` method returns a result containing the endpoint or an error if the handler setting fails.
    // The endpoint with the handler set is assigned to the `endpoint` variable.
    let endpoint = endpoint.set_handler(|request| {
        // Create an HTML response
        // The `create_html_response` function is called to create an HTML response.
        // The HTML response is assigned to the `response` variable.
        let response = create_html_response();

        // Return the response
        // The response is returned from the handler closure.
        // The response is returned as an `Ok` variant of the `Result` enum.
        // The `Ok` variant contains the response.
        Ok(response)
    });

    // Start the server
    // The `start` method is called on the `server` instance to start the server.
    // The `start` method returns a result containing the server instance or an error if the server fails to start.
    // The server instance is assigned to the `server` variable.
    let server = server.start()?;

    // Print the server address
    // The `get_address` method is called on the `server` instance to get the server address.
    // The server address is printed to the console.
    println!("Server started at {}", server.get_address());

    // Return the server instance
    // The server instance is returned from the `configure_webserver` function.
    // The server instance is returned as an `Ok` variant of the `Result` enum.
    // The `Ok` variant contains the server instance.
    return Ok(server);
}

fn create_html_response() -> String<256> {
    // Create an HTML Response with two buttons and send different messages to MQTT Broker
    let html = r#"
     <html>
         <body>
             <h1>ESP32 Web Server</h1>
             <button onclick="send('Hello')">Hello</button>
             <button onclick="send('World')">World</button>
             <script>
                 function send(message) {
                     fetch('/api/hello', {
                         method: 'GET',
                         headers: {
                             'Content-Type': 'application/json'
                         },
                         body: JSON.stringify({ message: message })
                     });
                 }
             </script>
         </body>
     </html>
 "#;
    let var_name = String::from(html);
    return var_name;
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello Medium!")
}
