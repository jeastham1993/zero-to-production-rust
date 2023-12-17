output "db_instance_address" {
  description = "The address of the RDS instance"
  value       = module.db.db_instance_address
}

output "db_instance_endpoint" {
  description = "The connection endpoint"
  value       = module.db.db_instance_endpoint
}

output "db_instance_engine" {
  description = "The database engine"
  value       = module.db.db_instance_engine
}

output "db_instance_identifier" {
  description = "The RDS instance identifier"
  value       = module.db.db_instance_identifier
}

output "db_instance_status" {
  description = "The RDS instance status"
  value       = module.db.db_instance_status
}

output "db_instance_name" {
  description = "The database name"
  value       = module.db.db_instance_name
}

output "db_instance_username" {
  description = "The master username for the database"
  value       = module.db.db_instance_username
  sensitive   = true
}

output "db_instance_port" {
  description = "The database port"
  value       = module.db.db_instance_port
}

output "db_instance_master_user_secret_arn" {
  description = "The ARN of the master user secret (Only available when manage_master_user_password is set to true)"
  value       = module.db.db_instance_master_user_secret_arn
}