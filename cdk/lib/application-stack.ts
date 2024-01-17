import * as cdk from 'aws-cdk-lib';
import { AttributeType, BillingMode, ProjectionType, StreamViewType, Table } from 'aws-cdk-lib/aws-dynamodb';
import { DockerImageAsset } from 'aws-cdk-lib/aws-ecr-assets';
import { ContainerImage } from 'aws-cdk-lib/aws-ecs';
import { ApplicationLoadBalancedFargateService } from 'aws-cdk-lib/aws-ecs-patterns';
import { Architecture, DockerImageCode, DockerImageFunction, StartingPosition } from 'aws-cdk-lib/aws-lambda';
import { DynamoEventSource, StreamEventSource } from 'aws-cdk-lib/aws-lambda-event-sources';
import { Bucket } from 'aws-cdk-lib/aws-s3';
import { Construct } from 'constructs';
import { NewsletterApi } from './api-stack';

export class Zero2ProdApplicationStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    let api = new NewsletterApi(this, "NewsletterApi");


    const send_confirmation_function = new DockerImageFunction(this, "SendConfirmationFunction", {
      code: DockerImageCode.fromImageAsset("../src/backend/", {
        file: "Dockerfile-SendConfirmation"
      }),
      architecture: Architecture.ARM_64
    });

    send_confirmation_function.addEventSource(new DynamoEventSource(api.NewsletterTable, {
      startingPosition: StartingPosition.TRIM_HORIZON
    }));

    const send_newsletter_function = new DockerImageFunction(this, "SendNewsletterFunction", {
      code: DockerImageCode.fromImageAsset("../src/backend/", {
        file: "Dockerfile-SendNewsletter"
      }),
      environment: {
        "APP_DATABASE__DATABASE_NAME": api.NewsletterTable.tableName,
        "APP_DATABASE__NEWSLETTER_STORAGE_BUCKET": api.NewsletterStorageBucket.bucketName,
      },
      architecture: Architecture.ARM_64
    });

    send_newsletter_function.addEventSource(new DynamoEventSource(api.NewsletterTable, {
      startingPosition: StartingPosition.TRIM_HORIZON
    }));

    api.NewsletterStorageBucket.grantRead(send_newsletter_function);
    api.NewsletterTable.grantReadData(send_newsletter_function);
  }
}
