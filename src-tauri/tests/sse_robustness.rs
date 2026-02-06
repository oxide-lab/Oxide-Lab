use bytes::Bytes;
use eventsource_stream::Eventsource;
use futures_util::{StreamExt, stream};

#[tokio::test]
async fn eventsource_handles_chunked_payloads() {
    let raw = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n",
        "data: [DONE]\n\n"
    );

    // Break the payload into awkward chunks to simulate TCP chunking.
    let chunks = vec![
        Bytes::from(raw[0..13].to_string()),
        Bytes::from(raw[13..41].to_string()),
        Bytes::from(raw[41..64].to_string()),
        Bytes::from(raw[64..raw.len()].to_string()),
    ];

    let input_stream = stream::iter(chunks.into_iter().map(Ok::<Bytes, std::io::Error>));
    let mut es = input_stream.eventsource();

    let mut events = Vec::new();
    while let Some(item) = es.next().await {
        let event = item.expect("eventsource item");
        events.push(event.data);
    }

    assert_eq!(events.len(), 3);
    assert_eq!(
        events[0],
        "{\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}"
    );
    assert_eq!(
        events[1],
        "{\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}"
    );
    assert_eq!(events[2], "[DONE]");
}

#[tokio::test]
async fn eventsource_handles_multiple_events_in_single_chunk() {
    let raw = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    let input_stream = stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::from(raw))]);
    let mut es = input_stream.eventsource();

    let mut events = Vec::new();
    while let Some(item) = es.next().await {
        let event = item.expect("eventsource item");
        events.push(event.data);
    }

    assert_eq!(
        events,
        vec![
            "{\"choices\":[{\"delta\":{\"content\":\"a\"}}]}".to_string(),
            "{\"choices\":[{\"delta\":{\"content\":\"b\"}}]}".to_string(),
            "[DONE]".to_string(),
        ]
    );
}
