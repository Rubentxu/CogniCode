variable "bucket_name" {
  type = string
}

variable "cidr" {
  type    = string
  default = "10.0.0.0/16"
}

output "bucket_id" {
  value = aws_s3_bucket.logs.id
}
