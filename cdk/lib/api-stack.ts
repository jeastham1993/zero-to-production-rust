import * as cdk from 'aws-cdk-lib';
import { Certificate } from 'aws-cdk-lib/aws-certificatemanager';
import { AttributeType, BillingMode, ProjectionType, StreamViewType, Table } from 'aws-cdk-lib/aws-dynamodb';
import { IVpc } from 'aws-cdk-lib/aws-ec2';
import { Architecture, DockerImageCode, DockerImageFunction, IFunction} from 'aws-cdk-lib/aws-lambda';
import { Bucket } from 'aws-cdk-lib/aws-s3';
import { Construct } from 'constructs';
import { LambdaIntegration, RestApi } from 'aws-cdk-lib/aws-apigateway';
import { getParameter } from '@aws-lambda-powertools/parameters/ssm';

export class NewsletterApi extends Construct {

  NewsletterTable: Table;
  NewsletterStorageBucket: Bucket;
  ApplicationVpc: IVpc;
  ApiFunction: IFunction;

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

    this.ApiFunction = new DockerImageFunction(this, "ApiFunction", {
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

    const baseDomain = process.env.BASE_DOMAIN ?? "";
    const certificateArn = process.env.CERTIFICATE_ARN ?? ""; 

    if (baseDomain.length > 0 && certificateArn.length > 0) {
      api.addDomainName("main-domain", {
        domainName: baseDomain,
        certificate: Certificate.fromCertificateArn(this, "Certificate", certificateArn)
      })
    }

    const newsletter_app_integration = new LambdaIntegration(this.ApiFunction, {
      requestTemplates: { "application/json": '{ "statusCode": "200" }' }
    });

    var proxyResource = api.root.addResource("{proxy+}");
    proxyResource.addMethod("ANY", newsletter_app_integration);

    this.NewsletterTable.grantReadWriteData(this.ApiFunction);
    auth_table.grantReadWriteData(this.ApiFunction);
    this.NewsletterStorageBucket.grantPut(this.ApiFunction);
}
}
