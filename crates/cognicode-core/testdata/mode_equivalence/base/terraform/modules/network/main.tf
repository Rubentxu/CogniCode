resource "aws_s3_bucket" "flow_logs" {
  bucket = "network-flow-logs"
}

output "flow_logs_bucket" {
  value = aws_s3_bucket.flow_logs.id
}
