import * as cdk from 'aws-cdk-lib';
import { Construct } from 'constructs';
import { NewsletterApi } from './api-stack';
import { NewSubscriberProcessingStack } from './new-subscriber-stack';
import { SendNewsletterProcessingStack } from './send-newsletter-stack';
import * as fs from "fs";
import * as ssm from "aws-cdk-lib/aws-ssm";
export class Zero2ProdApplicationStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);
    const api = new NewsletterApi(this, "NewsletterApi");

    var productionConfiguration = fs.readFileSync('../src/backend/configuration/production.yaml', 'utf8');

    const configParameter = new ssm.StringParameter(this, "ConfigParameter", {
      stringValue: productionConfiguration
    });
    
    const newSubscriberProcessor = new NewSubscriberProcessingStack(this, "NewSubscriberProcessor", {
      newsletterTable: api.NewsletterTable,
      newsletterStorageBucket: api.NewsletterStorageBucket,
      configParameter
    });
    
    const sendNewsletterProcessor = new SendNewsletterProcessingStack(this, "SendNewsletterProcessor", {
      newsletterTable: api.NewsletterTable,
      newsletterStorageBucket: api.NewsletterStorageBucket,
      configParameter
    });
  }
}
