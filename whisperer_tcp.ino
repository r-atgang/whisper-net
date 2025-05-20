// SPDX-License-Identifier: Apache-2.0
//
// Copyright 2025 ratgang
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//

#include <WiFi.h>
#include <WiFiClient.h>
#include <driver/i2s.h>

// === Network Configuration ===
const char* ssid_c2 = "NETGEAR69"; // Replace with your Wi-Fi SSID
const char* password_c2 = "ChangeThis000$"; // Replace with your Wi-Fi password
const char* address_TCP = "10.42.0.1"; // Replace with laptop/server IP
const int port_TCP = 6969; // TCP port for audio data

// === I2S Mic Configuration ===
#define ws_pin     18  // LRCLK / WS
#define sck_pin    19  // BCLK / SCK
#define sd_pin     22  // DOUT

const int SAMPLE_RATE = 16000;
const int buffer_samples = 1024;

WiFiClient client;

void setup() {
  Serial.begin(115200);

  // === Connect to Wi-Fi ===
  WiFi.begin(ssid_c2, password_c2);
  Serial.print("Connecting to Wi-Fi");
  while (WiFi.status() != WL_CONNECTED) {
    delay(500);
    Serial.print(".");
  }
  Serial.println("\nWi-Fi connected.");
  Serial.print("ESP32 IP: ");
  Serial.println(WiFi.localIP());

  // === Configure I2S ===
  const i2s_config_t i2s_config = {
    .mode = (i2s_mode_t)(I2S_MODE_MASTER | I2S_MODE_RX),
    .sample_rate = SAMPLE_RATE,
    .bits_per_sample = I2S_BITS_PER_SAMPLE_32BIT,
    .channel_format = I2S_CHANNEL_FMT_ONLY_LEFT,
    .communication_format = I2S_COMM_FORMAT_STAND_I2S,
    .intr_alloc_flags = ESP_INTR_FLAG_LEVEL1,
    .dma_buf_count = 4,
    .dma_buf_len = buffer_samples,
    .use_apll = false,
    .tx_desc_auto_clear = false,
    .fixed_mclk = -1
  };

  const i2s_pin_config_t pin_config = {
    .bck_io_num = sck_pin,
    .ws_io_num = ws_pin,
    .data_out_num = I2S_PIN_NO_CHANGE,
    .data_in_num = sd_pin
  };

  if (i2s_driver_install(I2S_NUM_0, &i2s_config, 0, NULL) != ESP_OK) {
    Serial.println("Error setting up I2S driver.");
    while (true);
  }

  if (i2s_set_pin(I2S_NUM_0, &pin_config) != ESP_OK) {
    Serial.println("Error setting I2S pins.");
    while (true);
  }

  Serial.println("I2S initialized successfully.");
}

void loop() {
  // Try to connect if not already connected
  if (!client.connected()) {
    Serial.print("Connecting to TCP server at ");
    Serial.print(address_TCP);
    Serial.print(":");
    Serial.println(port_TCP);

    if (client.connect(address_TCP, port_TCP)) {
      Serial.println("TCP connection established.");
    } else {
      Serial.println("TCP connection failed. Retrying in 1 second...");
      delay(1000);
      return;
    }
  }

  static int32_t samples[buffer_samples];       // 32-bit raw samples
  static int16_t samples_16bit[buffer_samples]; // 16-bit downsampled

  size_t bytes_read = 0;
  unsigned long packet_start_time = millis(); // Record time before reading

  // Read data from I2S
  i2s_read(I2S_NUM_0, samples, sizeof(samples), &bytes_read, portMAX_DELAY);
  unsigned long packet_end_time = millis(); // Time after reading

  // Check if packet is delayed more than 1 second
  if (packet_end_time - packet_start_time > 100) {
    Serial.println("Packet delayed >200ms, discarding...");
    return;
  }

  if (bytes_read > 0) {
    int num_samples = bytes_read / sizeof(int32_t);

    // Convert 32-bit signed samples to 16-bit
    for (int i = 0; i < num_samples; i++) {
      samples_16bit[i] = samples[i] >> 16;
    }

    // Send data over TCP
    size_t bytes_to_send = num_samples * sizeof(int16_t);
    size_t bytes_sent = client.write((uint8_t*)samples_16bit, bytes_to_send);

    if (bytes_sent != bytes_to_send) {
      Serial.print("Only sent ");
      Serial.print(bytes_sent);
      Serial.print(" of ");
      Serial.print(bytes_to_send);
      Serial.println(" bytes. Closing connection.");
      client.stop();
    } else {
      Serial.print("Sent ");
      Serial.print(bytes_sent);
      Serial.println(" bytes over TCP.");
    }
  }
}
