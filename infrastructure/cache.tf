resource "aws_elasticache_subnet_group" "session_cluster_subnets" {
  name       = "session-cluster-subnets"
  subnet_ids = module.vpc.private_subnets
}

resource "aws_elasticache_cluster" "session_clustser" {
  cluster_id           = "session-cluster"
  engine               = "redis"
  node_type            = "cache.t3.micro"
  num_cache_nodes      = 1
  parameter_group_name = "default.redis7"
  engine_version       = "7.1"
  port                 = 6379
  subnet_group_name    = aws_elasticache_subnet_group.session_cluster_subnets.name
  security_group_ids = [module.redis_security_group.security_group_id]
}