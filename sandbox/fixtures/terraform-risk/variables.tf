variable "environment" {
  type    = string
  default = "dev"
}

variable "bucket_acl" {
  type = string
}

variable "ssh_cidr" {
  type = string
}

variable "encrypt_volumes" {
  type = bool
}

variable "audit_enabled" {
  type = bool
}

variable "admin_action" {
  type = string
}
