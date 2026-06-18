# ─── Outputs ────────────────────────────────────────────────────────────────

output "vpc_id" {
  description = "ID of the VPC"
  value       = module.vpc.vpc_id
}

output "public_subnet_ids" {
  description = "Public subnet IDs"
  value       = module.vpc.public_subnet_ids
}

output "private_subnet_ids" {
  description = "Private subnet IDs"
  value       = module.vpc.private_subnet_ids
}

output "alb_dns_name" {
  description = "DNS name of the Application Load Balancer"
  value       = aws_lb.app.dns_name
}

output "alb_zone_id" {
  description = "Zone ID of the ALB for Route53 alias"
  value       = aws_lb.app.zone_id
}

output "db_address" {
  description = "RDS database address"
  value       = aws_db_instance.postgres.address
  sensitive   = true
}

output "db_port" {
  description = "RDS database port"
  value       = aws_db_instance.postgres.port
}

output "s3_bucket_name" {
  description = "S3 app bucket name"
  value       = aws_s3_bucket.app_bucket.id
}

output "cloudwatch_log_group" {
  description = "CloudWatch log group name"
  value       = aws_cloudwatch_log_group.app.name
}

output "ecs_cluster_name" {
  description = "ECS cluster name"
  value       = module.compute.ecs_cluster_name
}

output "app_task_role_arn" {
  description = "ARN of the app task IAM role"
  value       = aws_iam_role.app_task.arn
}
