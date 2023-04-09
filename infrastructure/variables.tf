variable "region" {
  description = "AWS Region"
  type        = string
  default     = "us-west-2"
}

variable "app_prefix" {
  description = "Name to prefix all resources with"
  type        = string
  default     = "rust-image-example"
}
