pub fn h264_sdp_stub() -> String {
    "v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\ns=Sunbeam\r\nt=0 0\r\nm=video 0 RTP/AVP 96\r\na=rtpmap:96 H264/90000\r\n".to_string()
}
