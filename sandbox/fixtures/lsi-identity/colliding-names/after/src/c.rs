pub fn handle(req: &Req) -> u32 {
    log_req(req);
    route_req(req);
    0
}
