import * as cdk from 'aws-cdk-lib';
import { Certificate } from 'aws-cdk-lib/aws-certificatemanager';
import { AttributeType, BillingMode, ProjectionType, StreamViewType, Table } from 'aws-cdk-lib/aws-dynamodb';
import { IVpc } from 'aws-cdk-lib/aws-ec2';
import { DockerImageAsset } from 'aws-cdk-lib/aws-ecr-assets';
import { ContainerImage, LogDrivers } from 'aws-cdk-lib/aws-ecs';
import { ApplicationLoadBalancedFargateService } from 'aws-cdk-lib/aws-ecs-patterns';
import { Architecture, DockerImageCode, DockerImageFunction, StartingPosition } from 'aws-cdk-lib/aws-lambda';
import { DynamoEventSource, StreamEventSource } from 'aws-cdk-lib/aws-lambda-event-sources';
import { RetentionDays } from 'aws-cdk-lib/aws-logs';
import { Bucket } from 'aws-cdk-lib/aws-s3';
import { Construct } from 'constructs';
import * as ecs from 'aws-cdk-lib/aws-ecs'
import { LambdaIntegration, RestApi } from 'aws-cdk-lib/aws-apigateway';
// import * as sqs from 'aws-cdk-lib/aws-sqs';

export class NewsletterApi extends Construct {

  NewsletterTable: Table;
  NewsletterStorageBucket: Bucket;
  ApplicationVpc: IVpc;

  constructor(scope: Construct, id: string) {
    super(scope, id);

    this.NewsletterTable = new Table(this, "NewsletterStorageFunction", {
        partitionKey: {
          name: "PK",
          type: AttributeType.STRING
        },
        billingMode: BillingMode.PAY_PER_REQUEST,
        stream: StreamViewType.NEW_IMAGE,
        removalPolicy: cdk.RemovalPolicy.DESTROY
      });
  
      this.NewsletterTable.addGlobalSecondaryIndex({
        indexName: "GSI1",
        partitionKey: {
          name: "GSI1PK",
          type: AttributeType.STRING
        },
        sortKey: {
          name: "GSI1SK",
          type: AttributeType.STRING
        },
        projectionType: ProjectionType.KEYS_ONLY
      })
  
      const auth_table = new Table(this, "NewsletterAuthTable", {
        partitionKey: {
          name: "PK",
          type: AttributeType.STRING
        },
        billingMode: BillingMode.PAY_PER_REQUEST,
        removalPolicy: cdk.RemovalPolicy.DESTROY
      });
  
      this.NewsletterStorageBucket = new Bucket(this, "NewsletterStorage", {
        bucketName: "james-eastham-newsletter-metadata",
        removalPolicy: cdk.RemovalPolicy.DESTROY
      });

    const api_function = new DockerImageFunction(this, "ApiFunction", {
      code: DockerImageCode.fromImageAsset("../src/api/", {
        file: "Dockerfile"
      }),
      architecture: Architecture.ARM_64,
      environment: {
        "APP_TELEMETRY__DATASET_NAME": "zero2prod-api",
        "APP_DATABASE__DATABASE_NAME": this.NewsletterTable.tableName,
        "APP_DATABASE__AUTH_DATABASE_NAME": auth_table.tableName,
        "APP_DATABASE__NEWSLETTER_STORAGE_BUCKET": this.NewsletterStorageBucket.bucketName,
        "LOG_LEVEL": "error",
        "AWS_LWA_REMOVE_BASE_PATH": "/prod"
      },
      memorySize: 256
    });

    const api = new RestApi(this, "NewsletterRestApi", {
      restApiName: "NewsletterApi",
      description: "This service serves widgets."
    });

    const newsletter_app_integration = new LambdaIntegration(api_function, {
      requestTemplates: { "application/json": '{ "statusCode": "200" }' }
    });

    var proxyResource = api.root.addResource("{proxy+}");
    proxyResource.addMethod("ANY", newsletter_app_integration);

    this.NewsletterTable.grantReadWriteData(api_function);
    auth_table.grantReadWriteData(api_function);
    this.NewsletterStorageBucket.grantPut(api_function);
  }
}
