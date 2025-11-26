// Copyright The OpenTelemetry Authors
// SPDX-License-Identifier: Apache-2.0

use actix_web::{post, web, HttpResponse, Responder};
use tracing::{info, error};
use opentelemetry::trace::TraceContextExt;
use opentelemetry::Context;
use serde_json::json;
use chrono::Utc;

mod quote;
use quote::create_quote_from_count;

mod tracking;
use tracking::create_tracking_id;

mod shipping_types;
pub use shipping_types::*;

const NANOS_MULTIPLE: u32 = 10000000u32;

#[post("/get-quote")]
pub async fn get_quote(req: web::Json<GetQuoteRequest>) -> impl Responder {
    let itemct: u32 = req.items.iter().map(|item| item.quantity as u32).sum();
    
    // Get current OpenTelemetry context and extract trace information
    let current_context = Context::current();
    let current_span = current_context.span();
    let span_context = current_span.span_context();
    let trace_id = span_context.trace_id().to_string();
    let span_id = span_context.span_id().to_string();
    
    let timestamp = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    
    // Log the incoming request
    let log_entry = json!({
        "time": timestamp,
        "trace_id": trace_id,
        "span_id": span_id,
        "message": format!("{:?}", *req),
    });
    info!("{}", log_entry.to_string());

    let quote = match create_quote_from_count(itemct).await {
        Ok(q) => q,
        Err(e) => {
            let log_entry_failed = json!({
                "time": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                "trace_id": trace_id,
                "span_id": span_id,
                "message": format!("GetQuoteRequest failed, error: {:?}", e),
            });
            error!("{}", log_entry_failed.to_string());
            return HttpResponse::InternalServerError().body(format!("Failed to get quote: {}", e));
        }
    };

    let log_entry_success = json!({
        "time": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "trace_id": trace_id,
        "span_id": span_id,
        "message": "GetQuoteRequest successfully",
    });
    info!("{}", log_entry_success.to_string());

    let reply = GetQuoteResponse {
        cost_usd: Some(Money {
            currency_code: "USD".into(),
            units: quote.dollars,
            nanos: quote.cents * NANOS_MULTIPLE,
        }),
    };

    let timestamp2 = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let log_entry2 = json!({
        "time": timestamp2,
        "trace_id": trace_id,
        "span_id": span_id,
        "message": format!("Sending Quote::{}", quote),
    });
    info!("{}", log_entry2.to_string());

    info!(
        name = "SendingQuoteValue",
        quote.dollars = quote.dollars,
        quote.cents = quote.cents,
        message = "Sending Quote"
    );

    HttpResponse::Ok().json(reply)
}

#[post("/ship-order")]
pub async fn ship_order(req: web::Json<ShipOrderRequest>) -> impl Responder {
    // Get current OpenTelemetry context and extract trace information
    let current_context = Context::current();
    let current_span = current_context.span();
    let span_context = current_span.span_context();
    let trace_id = span_context.trace_id().to_string();
    let span_id = span_context.span_id().to_string();
    
    let timestamp = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    
    // Log the incoming request
    let log_entry = json!({
        "time": timestamp,
        "trace_id": trace_id,
        "span_id": span_id,
        "message": format!("ShipOrderRequest: {:?}", *req),
    });
    info!("{}", log_entry.to_string());

    let tid = create_tracking_id();
    
    let timestamp2 = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let log_entry2 = json!({
        "time": timestamp2,
        "trace_id": trace_id,
        "span_id": span_id,
        "message": format!("Tracking ID Created: {}", tid),
    });
    info!("{}", log_entry2.to_string());
    
    info!(
        name = "CreatingTrackingId",
        tracking_id = tid.as_str(),
        message = "Tracking ID Created"
    );
    
    HttpResponse::Ok().json(ShipOrderResponse { tracking_id: tid })
}

#[cfg(test)]
mod tests {
    use actix_web::{http::header::ContentType, test, App};

    use super::*;

    #[actix_web::test]
    async fn test_ship_order() {
        let app = test::init_service(App::new().service(ship_order)).await;
        let req = test::TestRequest::post()
            .uri("/ship-order")
            .insert_header(ContentType::json())
            .set_json(&ShipOrderRequest {})
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let order: ShipOrderResponse = test::read_body_json(resp).await;
        assert!(!order.tracking_id.is_empty());
    }
}
